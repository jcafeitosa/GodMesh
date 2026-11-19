use std::{
    fs,
    io::Write,
    num::{NonZeroU32, NonZeroUsize},
    os::unix::fs::OpenOptionsExt,
};

use ed25519_dalek::{Signer, SigningKey};
use godmesh_federation::{
    NodeIdentity,
    event::SignedEvent,
    snapshot::{
        SnapshotError::{self, *},
        SnapshotIdentity, SnapshotLimits, SnapshotManifest, SnapshotPolicy, VerifiedSnapshot,
        encode_package, events_digest, verify_snapshot,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const VECTOR: &str = include_str!("fixtures/m4-b1-vetor.json");

fn fixture() -> Value {
    serde_json::from_str(VECTOR).unwrap()
}

fn fixture_hex(field: &str) -> Vec<u8> {
    hex::decode(fixture()[field].as_str().unwrap()).unwrap()
}

fn original_events() -> [Vec<u8>; 2] {
    let fixture = fixture();
    [0, 1].map(|index| hex::decode(fixture["events"][index].as_str().unwrap()).unwrap())
}

fn policy() -> SnapshotPolicy {
    let fixture = fixture();
    let manifest = &fixture["manifest"];
    let string = |field: &str| manifest[field].as_str().unwrap().to_owned();
    SnapshotPolicy {
        cluster: string("cluster"),
        origin: SnapshotIdentity {
            node_id: string("origin_node"),
            public_key: fixture["origin_public_key"].as_str().unwrap().into(),
        },
        destination: SnapshotIdentity {
            node_id: string("destination_node"),
            public_key: fixture["destination_public_key"].as_str().unwrap().into(),
        },
        tenant_id: string("tenant_id"),
        stream_id: string("stream_id"),
        partition: 0,
        authority_epoch: 7,
        retention_class: fixture["retention_class"].as_str().unwrap().into(),
        limits: SnapshotLimits {
            max_package_bytes: NonZeroUsize::new(1596).unwrap(),
            max_manifest_bytes: NonZeroU32::new(609).unwrap(),
            max_event_bytes: NonZeroU32::new(488).unwrap(),
            max_events: NonZeroU32::new(2).unwrap(),
        },
    }
}

fn roomy() -> SnapshotPolicy {
    let mut policy = policy();
    policy.limits.max_package_bytes = NonZeroUsize::new(32768).unwrap();
    policy.limits.max_manifest_bytes = NonZeroU32::new(4096).unwrap();
    policy.limits.max_event_bytes = NonZeroU32::new(4096).unwrap();
    policy
}

fn baseline() -> SnapshotManifest {
    verify_snapshot(&fixture_hex("package"), &policy())
        .unwrap()
        .manifest()
        .clone()
}

fn append_frame(out: &mut Vec<u8>, frame: &[u8]) {
    out.extend_from_slice(&u32::try_from(frame.len()).unwrap().to_be_bytes());
    out.extend_from_slice(frame);
}

fn pack(manifest: &[u8], events: &[&[u8]]) -> Vec<u8> {
    let mut package = Vec::new();
    append_frame(&mut package, manifest);
    for event in events {
        append_frame(&mut package, event);
    }
    package
}

fn verify(package: &[u8]) -> Result<VerifiedSnapshot<'_>, SnapshotError> {
    verify_snapshot(package, &roomy())
}

fn assert_recovery(package: &[u8], error: SnapshotError) {
    let policy = roomy();
    assert_eq!(verify_snapshot(package, &policy), Err(error));
    assert_eq!(
        verify_snapshot(&fixture_hex("package"), &policy)
            .unwrap()
            .cut(),
        (0, 2)
    );
}

// Semente sintética pública exclusiva da fixture independente.
fn origin_identity() -> NodeIdentity {
    let nonce = NodeIdentity::generate_in_memory().unwrap().public_key_hex();
    let path = std::env::temp_dir().join(format!(
        "godmesh-snapshot-test-{}-{nonce}",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    writeln!(file, "{}", hex::encode([1; 32])).unwrap();
    drop(file);
    let identity = NodeIdentity::load(&path);
    fs::remove_file(path).unwrap();
    identity.unwrap()
}

fn resigned(mut manifest: SnapshotManifest, frames: &[&[u8]]) -> Vec<u8> {
    manifest.events_digest = hex::encode(events_digest(frames).unwrap());
    manifest.signature = hex::encode(
        SigningKey::from_bytes(&[1; 32])
            .sign(&manifest.canonical_bytes().unwrap())
            .to_bytes(),
    );
    pack(&serde_json::to_vec(&manifest).unwrap(), frames)
}

#[test]
fn p_independent_vector_and_encoder_match_literally() {
    let fixture = fixture();
    let package = fixture_hex("package");
    let events = original_events();
    let frames = events.each_ref().map(Vec::as_slice);
    let result = verify_snapshot(&package, &policy()).unwrap();
    assert_eq!(
        hex::encode(Sha256::digest(&package)),
        fixture["package_sha256"]
    );
    assert_eq!(result.window(), (100, 200));
    assert_eq!(result.cut(), (0, 2));
    assert_eq!(result.events(), &frames);
    let manifest_len = fixture_hex("manifest_bytes").len();
    assert_eq!(&package[4..4 + manifest_len], fixture_hex("manifest_bytes"));
    let reordered = fixture["manifest"]
        .as_object()
        .unwrap()
        .iter()
        .rev()
        .map(|(key, value)| format!("{}: {}", json!(key), value))
        .collect::<Vec<_>>()
        .join(", ");
    let reordered = pack(format!("{{ {reordered} }}").as_bytes(), &frames);
    assert_eq!(verify(&reordered).unwrap(), result);
    let mut offset = 4 + manifest_len;
    for (index, event) in result.events().iter().enumerate() {
        offset += 4;
        assert_eq!(event.as_ptr(), package[offset..].as_ptr());
        offset += event.len();
        let parsed: SignedEvent = serde_json::from_slice(event).unwrap();
        assert_eq!(
            hex::encode(parsed.canonical_bytes()),
            fixture["canonical_events"][index]
        );
        assert_eq!(parsed.signature, fixture["event_signatures"][index]);
    }

    let mut preimage = b"godmesh.federation.snapshot.events.v1".to_vec();
    preimage.extend_from_slice(&2_u32.to_be_bytes());
    for frame in frames {
        append_frame(&mut preimage, frame);
    }
    assert_eq!(preimage, fixture_hex("events_digest_input"));
    assert_eq!(
        hex::encode(events_digest(&frames).unwrap()),
        fixture["events_digest"]
    );

    let mut manifest = result.manifest().clone();
    assert_eq!(
        manifest.canonical_bytes().unwrap(),
        fixture_hex("canonical_manifest")
    );
    manifest.signature.clear();
    manifest.sign(&origin_identity()).unwrap();
    assert_eq!(manifest.signature, fixture["manifest_signature"]);
    assert_eq!(
        encode_package(&manifest, &frames, &policy().limits).unwrap(),
        package
    );
}

#[test]
fn n_manifest_structure_profile_policy_and_precedence() {
    let events = original_events();
    let frames = events.each_ref().map(Vec::as_slice);
    for (manifest, error) in [
        (
            r#"{"profile":2,"other":1e999,"other":2}"#,
            UnsupportedProfile,
        ),
        (r#"{"profile":18446744073709551615}"#, UnsupportedProfile),
        (r#"{"profile":2,"other":[1,]}"#, MalformedManifest),
        (r#"{"profile":2,"other":"\q"}"#, MalformedManifest),
        (r#"{"profile":2,"other":[1]"#, MalformedManifest),
        (r#"{"profile":2} {}"#, MalformedManifest),
        (r#"{"profile":2,"\u0070rofile":1}"#, MalformedManifest),
        (r#"{"profile":-0}"#, MalformedManifest),
        (r#"{"profile":-1}"#, MalformedManifest),
        (r#"{"profile":1.0}"#, MalformedManifest),
        (r#"{"profile":1e0}"#, MalformedManifest),
        (r#"{"profile":18446744073709551616}"#, MalformedManifest),
        (r#"{"extra":1}"#, MalformedManifest),
        (r#"[1]"#, MalformedManifest),
    ] {
        assert_recovery(&pack(manifest.as_bytes(), &frames), error);
    }

    let json = String::from_utf8(fixture_hex("manifest_bytes")).unwrap();
    for (from, to, error) in [
        (
            "\"base_seq\":0",
            "\"base_seq\":-0",
            InvalidField("base_seq"),
        ),
        (
            "\"authority_epoch\":7",
            "\"authority_epoch\":0",
            InvalidField("authority_epoch"),
        ),
        (
            "\"partition\":0",
            "\"partition\":1",
            InvalidField("partition"),
        ),
        ("\"cut_seq\":2", "\"cut_seq\":0", InvalidField("cut_seq")),
        (
            "\"cut_seq\":2",
            "\"cut_seq\":18446744073709551616",
            InvalidField("cut_seq"),
        ),
        (
            "\"event_count\":2",
            "\"event_count\":4294967296",
            InvalidField("event_count"),
        ),
        ("\"event_count\":2", "\"event_count\":1", InvalidCut),
        (
            "\"event_count\":2",
            "\"event_count\":2,\"event_count\":2",
            MalformedManifest,
        ),
        (
            "\"event_count\":2",
            "\"event_count\":2,\"extra\":0",
            MalformedManifest,
        ),
        ("\"event_count\":2,", "", MalformedManifest),
        ("\"base_seq\":0", "\"base_seq\":2", InvalidCut),
        ("\"base_seq\":0", "\"base_seq\":3", InvalidCut),
        ("\"expires_at\":200", "\"expires_at\":100", InvalidWindow),
        ("\"expires_at\":200", "\"expires_at\":99", InvalidWindow),
        (
            "\"expires_at\":200",
            "\"expires_at\":9223372036854775808",
            InvalidField("expires_at"),
        ),
        (
            "\"events_digest\":\"",
            "\"events_digest\":\"F",
            InvalidField("events_digest"),
        ),
        (
            "\"signature\":\"",
            "\"signature\":\"F",
            InvalidField("signature"),
        ),
    ] {
        assert!(json.contains(from));
        assert_recovery(&pack(json.replacen(from, to, 1).as_bytes(), &frames), error);
    }
    let wrong_type = json
        .replace("\"base_seq\":0", "\"base_seq\":2")
        .replace("\"not_before\":100", "\"not_before\":1e2");
    assert_recovery(&pack(wrong_type.as_bytes(), &frames), MalformedManifest);

    let package = fixture_hex("package");
    let mut policy = policy();
    policy.destination.public_key = policy.origin.public_key.clone();
    assert_eq!(verify_snapshot(&[], &policy), Err(InvalidPolicy));
    let mut policy = self::policy();
    let alternate = NodeIdentity::generate_in_memory().unwrap();
    policy.destination = SnapshotIdentity {
        node_id: alternate.node_id().into(),
        public_key: alternate.public_key_hex(),
    };
    assert_eq!(verify_snapshot(&package, &policy), Err(ScopeMismatch));
}

#[test]
fn n_framing_precedes_crypto_and_crypto_precedes_proof() {
    let positive = fixture_hex("package");
    let events = original_events();
    let frames = events.each_ref().map(Vec::as_slice);
    let manifest = fixture_hex("manifest_bytes");
    let changed_escape = String::from_utf8(events[1].clone())
        .unwrap()
        .replace(r"\u0076", "v");
    assert_ne!(changed_escape.as_bytes(), frames[1]);
    for changed in [
        [frames[0].strip_prefix(b" ").unwrap(), frames[1]],
        [frames[0], changed_escape.as_bytes()],
        [frames[0], b"{"],
    ] {
        assert_recovery(&pack(&manifest, &changed), EventsDigestMismatch);
    }

    let mut bad_sig = fixture()["manifest"].clone();
    bad_sig["signature"] = json!("0".repeat(128));
    let bad_sig = serde_json::to_vec(&bad_sig).unwrap();
    assert_recovery(
        &pack(&bad_sig, &[frames[0], b"{"]),
        ManifestSignatureInvalid,
    );
    let mut truncated = pack(&bad_sig, &frames);
    truncated.pop();
    assert_recovery(&truncated, MalformedPackage);

    assert_recovery(&0_u32.to_be_bytes(), MalformedManifest);
    assert_recovery(&u32::MAX.to_be_bytes(), ManifestTooLarge);
    assert_recovery(
        &pack(b"{\"profile\":2,\"x\":\"\xff\"}", &frames),
        MalformedManifest,
    );
    let mut oversized_event = pack(&manifest, &[]);
    oversized_event.extend_from_slice(&u32::MAX.to_be_bytes());
    assert_recovery(&oversized_event, EventTooLarge);

    let first = 4 + manifest.len();
    let second = first + 4 + frames[0].len();
    for len in [0, 1, 3, 4, first - 1] {
        assert_recovery(&positive[..len], MalformedManifest);
    }
    for len in [
        first,
        first + 1,
        first + 3,
        first + 4,
        second,
        second + 1,
        second + 3,
        second + 4,
        positive.len() - 1,
    ] {
        assert_recovery(&positive[..len], MalformedPackage);
    }
    assert_recovery(&pack(&manifest, &[b"", frames[1]]), MalformedPackage);
    let mut extra = positive;
    extra.push(b' ');
    assert_recovery(&extra, MalformedPackage);
}

#[test]
fn n_authenticated_bad_proofs_have_exact_index_order() {
    let events = original_events();
    let original: Value = serde_json::from_slice(&events[1]).unwrap();
    let mut cases = vec![
        (b"[]".to_vec(), InvalidEvent(1)),
        (b"{\"x\":\"\xff\"}".to_vec(), InvalidEvent(1)),
    ];
    for (field, value, error) in [
        ("position", json!([1, 2, 3, 4, 5]), InvalidEvent(1)),
        ("tenant_id", json!("x".repeat(257)), InvalidEvent(1)),
        ("tenant_id", json!("other-tenant"), EventScopeMismatch(1)),
        (
            "retention_class",
            json!("other-class"),
            EventScopeMismatch(1),
        ),
        ("node_id", json!("wrong"), InvalidEvent(1)),
        ("signature", json!("A".repeat(128)), InvalidEvent(1)),
        ("issued_at", json!(1.5), InvalidEvent(1)),
        ("extra", json!(null), InvalidEvent(1)),
    ] {
        let mut changed = original.clone();
        changed[field] = value;
        cases.push((serde_json::to_vec(&changed).unwrap(), error));
    }
    let mut missing = original.clone();
    missing.as_object_mut().unwrap().remove("signature");
    cases.push((serde_json::to_vec(&missing).unwrap(), InvalidEvent(1)));
    let mut nested_missing = original.clone();
    nested_missing["position"]
        .as_object_mut()
        .unwrap()
        .remove("seq");
    cases.push((
        serde_json::to_vec(&nested_missing).unwrap(),
        InvalidEvent(1),
    ));
    for (from, to) in [
        ("\"seq\":2", "\"seq\":2,\"seq\":2"),
        ("\"seq\":2", "\"seq\":2,\"extra\":0"),
        ("\"seq\":2", "\"seq\":9223372036854775808"),
        ("\"seq\":2", "\"seq\":-0"),
        ("\"authority_epoch\":7", "\"authority_epoch\":-0"),
        ("\"partition\":0", "\"partition\":-0"),
        ("\"issued_at\":90", "\"issued_at\":-9223372036854775809"),
        ("\"issued_at\":90", "\"issued_at\":90,\"issued_at\":90"),
    ] {
        let wire = String::from_utf8_lossy(&events[1]);
        assert!(wire.contains(from));
        cases.push((wire.replacen(from, to, 1).into_bytes(), InvalidEvent(1)));
    }
    let mut changed = original.clone();
    changed["position"]["seq"] = json!(1);
    cases.push((serde_json::to_vec(&changed).unwrap(), SequenceMismatch(1)));
    let mut changed = original;
    let signature = changed["signature"].as_str().unwrap();
    changed["signature"] = json!(format!("0{}", &signature[1..]));
    cases.push((
        serde_json::to_vec(&changed).unwrap(),
        EventSignatureInvalid(1),
    ));

    for (frame, error) in cases {
        assert_recovery(&resigned(baseline(), &[&events[0], &frame]), error);
    }

    let mut first: Value = serde_json::from_slice(&events[0]).unwrap();
    first["tenant_id"] = json!("other-tenant");
    let first = serde_json::to_vec(&first).unwrap();
    assert_recovery(
        &resigned(baseline(), &[&first, b"{"]),
        EventScopeMismatch(0),
    );
}

#[test]
fn b_exact_limits_large_count_without_frames_and_historical_windows() {
    let package = fixture_hex("package");
    let mut policy = policy();
    assert!(verify_snapshot(&package, &policy).is_ok());
    policy.limits.max_package_bytes = NonZeroUsize::new(package.len() - 1).unwrap();
    assert_eq!(verify_snapshot(&package, &policy), Err(PackageTooLarge));
    let mut policy = self::policy();
    policy.limits.max_manifest_bytes = NonZeroU32::new(608).unwrap();
    assert_eq!(verify_snapshot(&package, &policy), Err(ManifestTooLarge));
    let mut policy = self::policy();
    policy.limits.max_event_bytes = NonZeroU32::new(487).unwrap();
    assert_eq!(verify_snapshot(&package, &policy), Err(EventTooLarge));
    let mut policy = self::policy();
    policy.limits.max_events = NonZeroU32::new(1).unwrap();
    assert_eq!(verify_snapshot(&package, &policy), Err(TooManyEvents));

    let mut huge = baseline();
    huge.cut_seq = u64::from(u32::MAX);
    huge.event_count = u32::MAX;
    let mut policy = roomy();
    policy.limits.max_events = NonZeroU32::MAX;
    let huge = pack(&serde_json::to_vec(&huge).unwrap(), &[]);
    assert_eq!(verify_snapshot(&huge, &policy), Err(MalformedPackage));

    let events = original_events();
    let frames = events.each_ref().map(Vec::as_slice);
    for window in [(i64::MIN, -1), (0, i64::MAX - 1), (i64::MAX - 1, i64::MAX)] {
        let mut manifest = baseline();
        (manifest.not_before, manifest.expires_at) = window;
        let mut packet = resigned(manifest, &frames);
        if window.0 == 0 {
            let length = u32::from_be_bytes(packet[..4].try_into().unwrap()) as usize;
            let json = std::str::from_utf8(&packet[4..4 + length])
                .unwrap()
                .replace("\"not_before\":0", "\"not_before\":-0");
            packet = pack(json.as_bytes(), &frames);
        }
        assert_eq!(verify(&packet).unwrap().window(), window);
    }

    let mut manifest = baseline();
    manifest.cut_seq = i64::MAX as u64;
    manifest.base_seq = manifest.cut_seq - 1;
    manifest.event_count = 1;
    let mut event: SignedEvent = serde_json::from_slice(frames[0]).unwrap();
    event.position.seq = manifest.cut_seq;
    event.issued_at = 0;
    event.signature = hex::encode(
        SigningKey::from_bytes(&[1; 32])
            .sign(&event.canonical_bytes())
            .to_bytes(),
    );
    let event = serde_json::to_string(&event)
        .unwrap()
        .replace("\"issued_at\":0", "\"issued_at\":-0");
    let packet = resigned(manifest, &[event.as_bytes()]);
    assert_eq!(
        verify(&packet).unwrap().cut(),
        (i64::MAX as u64 - 1, i64::MAX as u64)
    );
}
