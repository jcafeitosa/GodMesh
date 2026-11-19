use std::num::NonZeroUsize;

use ed25519_dalek::{Signer, SigningKey};
use godmesh_federation::config_bundle::{
    AuthenticatedBundle,
    BundleError::{self, *},
    BundlePolicy, BundleStatus, HistoryRecord, HistoryStatus, PinnedVoter, SoftwareVersion,
    authenticate_bundle, vote_bytes,
};
use serde_json::{Value, json};

fn fixture(size: u64) -> Value {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/m3-a-vetor.json")).unwrap();
    fixture[format!("quorum_{size}")].clone()
}

fn policy(size: u64) -> BundlePolicy {
    let fixture = fixture(size);
    BundlePolicy {
        cluster: "cluster-teste".into(),
        epoch: 7,
        max_frame_bytes: NonZeroUsize::new(32768).unwrap(),
        voters: fixture["voters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|voter| PinnedVoter {
                node_id: voter["node_id"].as_str().unwrap().into(),
                public_key: voter["public_key"].as_str().unwrap().into(),
            })
            .collect(),
    }
}

fn frame(size: u64) -> Value {
    fixture(size)["bundle"].clone()
}

fn authenticate(frame: &Value) -> Result<AuthenticatedBundle, BundleError> {
    authenticate_bundle(&serde_json::to_vec(frame).unwrap(), &policy(3))
}

fn raw(frame: &str) -> Result<AuthenticatedBundle, BundleError> {
    authenticate_bundle(frame.as_bytes(), &policy(3))
}

fn baseline() -> AuthenticatedBundle {
    authenticate(&frame(3)).unwrap()
}

fn software(major: u64, minor: u64, patch: u64) -> SoftwareVersion {
    SoftwareVersion::new(major, minor, patch).unwrap()
}

fn signatures(value: &mut Value, hash: &[u8; 32]) {
    for (index, vote) in value["signatures"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        let seed = u8::try_from(index + 1).unwrap();
        let signer = SigningKey::from_bytes(&[seed; 32]);
        vote["signature"] = json!(hex::encode(signer.sign(&vote_bytes(hash)).to_bytes()));
    }
}

fn rehash(value: &mut Value, body: &godmesh_federation::config_bundle::BundleBody) {
    let hash = body.digest();
    value["hash"] = json!(hex::encode(hash));
    signatures(value, &hash);
}

#[test]
fn p_independent_vectors_and_equivalent_json() {
    for size in [3, 5] {
        let vector = fixture(size);
        let mut bundle = vector["bundle"].clone();
        let policy = policy(size);
        let result = authenticate_bundle(&serde_json::to_vec(&bundle).unwrap(), &policy).unwrap();
        let canonical = result.body().canonical_bytes();
        assert_eq!(canonical.len(), 428);
        assert_eq!(hex::encode(&canonical), vector["canonical_hex"]);
        assert_eq!(hex::encode(result.hash()), bundle["hash"]);
        assert_eq!(result.body().digest(), *result.hash());
        assert_eq!(hex::encode(vote_bytes(result.hash())), vector["vote_hex"]);

        let frozen = bundle["signatures"].clone();
        signatures(&mut bundle, result.hash());
        assert_eq!(bundle["signatures"], frozen);
        bundle["signatures"].as_array_mut().unwrap().reverse();
        let fields = bundle
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| format!("{}: {}", json!(key), value))
            .collect::<Vec<_>>();
        let reordered = format!(
            "{{ {} }}",
            fields.into_iter().rev().collect::<Vec<_>>().join(", ")
        )
        .replacen("\"profile\"", "\"\\u0070rofile\"", 1)
        .replacen("cluster-teste", "cluster-\\u0074este", 1);
        assert_eq!(
            authenticate_bundle(reordered.as_bytes(), &policy),
            Ok(result)
        );
    }
}

#[test]
fn n_policy_and_frame_semantics_precede_crypto() {
    let mut invalid = policy(3);
    invalid.voters[1] = invalid.voters[0].clone();
    assert_eq!(authenticate_bundle(b"broken", &invalid), Err(InvalidPolicy));
    invalid = policy(3);
    invalid.voters[1].public_key = invalid.voters[0].public_key.clone();
    assert_eq!(authenticate_bundle(b"broken", &invalid), Err(InvalidPolicy));
    invalid = policy(3);
    invalid.voters.pop();
    assert_eq!(authenticate_bundle(b"broken", &invalid), Err(InvalidPolicy));
    invalid = policy(3);
    invalid.epoch = i64::MAX as u64 + 1;
    assert_eq!(authenticate_bundle(b"broken", &invalid), Err(InvalidPolicy));
    invalid = policy(3);
    invalid.cluster = " ".into();
    assert_eq!(authenticate_bundle(b"broken", &invalid), Err(InvalidPolicy));

    let original = frame(3);
    for (field, value, error) in [
        ("cluster", json!("outro-cluster"), ClusterMismatch),
        ("epoch", json!(8), EpochMismatch),
        ("namespace", json!("local"), InvalidField("namespace")),
        ("scope_id", json!("tenant"), InvalidField("scope_id")),
        (
            "required_signatures",
            json!(4),
            InvalidField("required_signatures"),
        ),
        ("entries", json!([]), InvalidField("entries")),
        ("hash", json!("F".repeat(64)), InvalidField("hash")),
        ("hash", json!("0".repeat(64)), HashMismatch),
    ] {
        let mut bundle = original.clone();
        bundle[field] = value;
        assert_eq!(authenticate(&bundle), Err(error));
    }
    let mut bundle = original.clone();
    bundle["entries"][0]["key"] = json!("local.other");
    assert_eq!(authenticate(&bundle), Err(InvalidField("key")));
    let mut bundle = original.clone();
    bundle["cluster"] = json!("outro-cluster");
    bundle["epoch"] = json!(8);
    assert_eq!(authenticate(&bundle), Err(ClusterMismatch));

    let mut bundle = original;
    bundle["hash"] = json!("0".repeat(64));
    let raw_bundle = serde_json::to_string(&bundle).unwrap();
    assert_eq!(raw(&raw_bundle), Err(HashMismatch));
    assert_eq!(
        raw(&raw_bundle.replace("\"version\":9", "\"version\":18446744073709551616")),
        Err(InvalidField("version"))
    );
    assert_eq!(
        raw(&raw_bundle
            .replace("\"version\":9", "\"version\":0")
            .replace("\"effective_after\":120", "\"effective_after\":1e2")),
        Err(MalformedFrame)
    );
}

#[test]
fn n_votes_threshold_and_order() {
    let mut bundle = frame(5);
    assert_eq!(authenticate(&bundle), Err(ThresholdMismatch));
    bundle["hash"] = json!("0".repeat(64));
    assert_eq!(authenticate(&bundle), Err(HashMismatch));

    let mut bundle = frame(3);
    bundle["signatures"][0]["signature"] = json!("0".repeat(128));
    bundle["signatures"][2] = bundle["signatures"][1].clone();
    assert_eq!(authenticate(&bundle), Err(DuplicateVote));

    for votes in [json!([]), json!([frame(3)["signatures"][0]])] {
        let mut bundle = frame(3);
        bundle["signatures"] = votes;
        assert_eq!(authenticate(&bundle), Err(InsufficientQuorum));
    }

    let mut bundle = frame(3);
    bundle["signatures"][0]["signer_node_id"] = json!(format!("gmn_{}", "f".repeat(64)));
    assert_eq!(authenticate(&bundle), Err(UnknownVoter));
    bundle["signatures"][1]["signature"] = json!("0".repeat(128));
    assert_eq!(authenticate(&bundle), Err(InvalidSignature));
    bundle["signatures"].as_array_mut().unwrap().reverse();
    assert_eq!(authenticate(&bundle), Err(InvalidSignature));

    let mut bundle = frame(3);
    bundle["applies_at"] = json!(10);
    bundle["expires_at"] = json!(30);
    bundle["entries"][0]["effective_after"] = json!(20);
    let mut body = baseline().body().clone();
    body.applies_at = 10;
    body.expires_at = 30;
    body.effective_after = 20;
    rehash(&mut bundle, &body);
    assert_eq!(
        authenticate(&bundle)
            .unwrap()
            .evaluate(200, software(1, 2, 3)),
        BundleStatus::Expired
    );
    bundle["signatures"][0]["signature"] = json!("0".repeat(128));
    assert_eq!(authenticate(&bundle), Err(InvalidSignature));
}

#[test]
fn n_trusted_history_is_independent_of_time() {
    let original = baseline();
    let mut history = HistoryRecord {
        key: original.body().key.clone(),
        version: original.body().version,
        hash: *original.hash(),
    };
    assert_eq!(original.compare_history(None), HistoryStatus::NewVersion);
    assert_eq!(
        original.compare_history(Some(&history)),
        HistoryStatus::Replay
    );
    for (key, change) in [
        ("cluster", "outro-cluster"),
        ("namespace", "tenant"),
        ("scope_id", "outro-escopo"),
    ] {
        let mut history = history.clone();
        match key {
            "cluster" => history.key.cluster = change.into(),
            "namespace" => history.key.namespace = change.into(),
            "scope_id" => history.key.scope_id = change.into(),
            _ => unreachable!(),
        }
        assert_eq!(
            original.compare_history(Some(&history)),
            HistoryStatus::HistoryKeyMismatch
        );
    }
    let mut wrong_epoch = history.clone();
    wrong_epoch.key.epoch += 1;
    assert_eq!(
        original.compare_history(Some(&wrong_epoch)),
        HistoryStatus::HistoryKeyMismatch
    );

    let mut bundle = frame(3);
    let mut body = original.body().clone();
    body.value += 1;
    bundle["entries"][0]["value"] = json!(body.value);
    rehash(&mut bundle, &body);
    let changed = authenticate(&bundle).unwrap();
    assert_eq!(
        changed.evaluate(200, software(1, 2, 3)),
        BundleStatus::Expired
    );
    assert_eq!(
        changed.compare_history(Some(&history)),
        HistoryStatus::Conflict
    );
    history.version += 1;
    assert_eq!(
        original.compare_history(Some(&history)),
        HistoryStatus::Rollback
    );
    history.version -= 2;
    assert_eq!(
        original.compare_history(Some(&history)),
        HistoryStatus::NewVersion
    );
}

#[test]
fn b_frame_time_and_software_limits() {
    let mut bytes = serde_json::to_vec(&frame(3)).unwrap();
    let mut policy = policy(3);
    policy.max_frame_bytes = NonZeroUsize::new(bytes.len()).unwrap();
    let result = baseline();
    assert_eq!(authenticate_bundle(&bytes, &policy), Ok(result.clone()));
    bytes.push(b' ');
    assert_eq!(authenticate_bundle(&bytes, &policy), Err(FrameTooLarge));
    assert_eq!(result.body().value, 2);
    for (now, status) in [
        (119, BundleStatus::Pending),
        (120, BundleStatus::Eligible),
        (199, BundleStatus::Eligible),
        (200, BundleStatus::Expired),
    ] {
        assert_eq!(result.evaluate(now, software(1, 2, 3)), status);
    }
    assert_eq!(
        result.evaluate(119, software(1, 2, 2)),
        BundleStatus::IncompatibleSoftware
    );
    assert_eq!(
        result.evaluate(200, software(0, 0, 0)),
        BundleStatus::Expired
    );
    assert!(SoftwareVersion::new(0, 0, i64::MAX as u64 + 1).is_none());

    let mut bundle = frame(3);
    let mut body = result.body().clone();
    body.key.epoch = 0;
    body.version = i64::MAX as u64;
    body.applies_at = 0;
    body.expires_at = i64::MAX;
    body.effective_after = i64::MIN;
    body.value = 1;
    body.min_version = software(0, 0, i64::MAX as u64);
    bundle["epoch"] = json!(body.key.epoch);
    bundle["version"] = json!(body.version);
    bundle["applies_at"] = json!(body.applies_at);
    bundle["expires_at"] = json!(body.expires_at);
    bundle["entries"][0]["effective_after"] = json!(body.effective_after);
    bundle["entries"][0]["value"] = json!(body.value);
    bundle["entries"][0]["min_version"] = json!(format!("0.0.{}", i64::MAX));
    rehash(&mut bundle, &body);
    policy = self::policy(3);
    policy.epoch = 0;
    let raw_bundle = serde_json::to_string(&bundle)
        .unwrap()
        .replace("\"applies_at\":0", "\"applies_at\":-0");
    let result = authenticate_bundle(raw_bundle.as_bytes(), &policy).unwrap();
    assert_eq!(result.body(), &body);
    assert_eq!(
        result.evaluate(i64::MIN, body.min_version),
        BundleStatus::Pending
    );
    assert_eq!(result.evaluate(0, body.min_version), BundleStatus::Eligible);
    assert_eq!(
        result.evaluate(i64::MAX, body.min_version),
        BundleStatus::Expired
    );
}

#[test]
fn b_lexical_integers_and_field_ranges() {
    let original = serde_json::to_string(&frame(3)).unwrap();
    for (field, old, new, error) in [
        ("profile", "1", "-0", MalformedFrame),
        ("profile", "1", "-1", MalformedFrame),
        ("profile", "1", "18446744073709551615", UnsupportedProfile),
        ("profile", "1", "18446744073709551616", MalformedFrame),
        ("version", "9", "-0", InvalidField("version")),
        ("version", "9", "0", InvalidField("version")),
        ("epoch", "7", "-0", InvalidField("epoch")),
        (
            "required_signatures",
            "3",
            "-0",
            InvalidField("required_signatures"),
        ),
        ("value", "2", "-0", InvalidField("value")),
        ("value", "2", "0", InvalidField("value")),
        ("value", "2", "9223372036854775808", InvalidField("value")),
        ("value", "2", "18446744073709551616", InvalidField("value")),
        ("value", "2", "2.0", MalformedFrame),
        ("value", "2", "2e0", MalformedFrame),
        ("value", "2", "02", MalformedFrame),
        ("value", "2", "+2", MalformedFrame),
        ("value", "2", "\"2\"", MalformedFrame),
        (
            "applies_at",
            "100",
            "-9223372036854775809",
            InvalidField("applies_at"),
        ),
        (
            "expires_at",
            "200",
            "9223372036854775808",
            InvalidField("expires_at"),
        ),
        ("expires_at", "200", "100", InvalidField("expires_at")),
        (
            "effective_after",
            "120",
            "200",
            InvalidField("effective_after"),
        ),
        (
            "min_version",
            "\"1.2.3\"",
            "\"01.2.3\"",
            InvalidField("min_version"),
        ),
        (
            "min_version",
            "\"1.2.3\"",
            "\"1.2.9223372036854775808\"",
            InvalidField("min_version"),
        ),
    ] {
        let needle = format!("\"{field}\":{old}");
        assert!(original.contains(&needle));
        let modified = original.replacen(&needle, &format!("\"{field}\":{new}"), 1);
        assert_eq!(raw(&modified), Err(error), "{field}: {new}");
    }
}

#[test]
fn f_profile_structural_objects_and_recovery() {
    let original = serde_json::to_string(&frame(3)).unwrap();
    let result = baseline();
    let mut frames = vec![
        (
            br#"{"profile":2,"entries":{"large":1e999,"votes":[null]}}"#.to_vec(),
            UnsupportedProfile,
        ),
        (
            br#"{"profile":2,"one":0,"one":1}"#.to_vec(),
            UnsupportedProfile,
        ),
        (
            br#"{"profile":2,"\u0070rofile":1}"#.to_vec(),
            MalformedFrame,
        ),
        (br#"{"profile":2,"one":"\q"}"#.to_vec(), MalformedFrame),
        (br#"{"profile":2,"one":[1,]}"#.to_vec(), MalformedFrame),
        (br#"{"profile":1.0}"#.to_vec(), MalformedFrame),
        (br#"{"profile":1e0}"#.to_vec(), MalformedFrame),
        (br#"{"profile":"1"}"#.to_vec(), MalformedFrame),
        (br#"{"other":1}"#.to_vec(), MalformedFrame),
        (br#"[2]"#.to_vec(), MalformedFrame),
        (b"{\"profile\":2,\"x\":\"\xff\"}".to_vec(), MalformedFrame),
        (
            original.as_bytes()[..original.len() - 1].to_vec(),
            MalformedFrame,
        ),
        (format!("{original} null").into_bytes(), MalformedFrame),
    ];
    for (old, replacement) in [
        ("\"value\":2", "\"value\":2,\"value\":2"),
        ("\"signature\":", "\"extra\":true,\"signature\":"),
        (
            "\"signature\":",
            "\"signer_node_id\":\"duplicate\",\"signature\":",
        ),
        ("\"profile\":1", "\"extra\":true,\"profile\":1"),
        ("\"effective_after\":120,", ""),
    ] {
        assert!(original.contains(old));
        frames.push((
            original.replacen(old, replacement, 1).into_bytes(),
            MalformedFrame,
        ));
    }
    let mut bundle = frame(3);
    bundle["entries"][0] = json!(["federation.minimum_protocol_version", 2, 120, "1.2.3"]);
    frames.push((serde_json::to_vec(&bundle).unwrap(), MalformedFrame));
    bundle = frame(3);
    let vote = &bundle["signatures"][0];
    bundle["signatures"][0] = json!([vote["signer_node_id"], vote["signature"]]);
    frames.push((serde_json::to_vec(&bundle).unwrap(), MalformedFrame));

    let policy = policy(3);
    for (frame, error) in frames {
        assert_eq!(
            authenticate_bundle(&frame, &policy),
            Err(error),
            "{}",
            String::from_utf8_lossy(&frame)
        );
        assert_eq!(
            authenticate_bundle(original.as_bytes(), &policy),
            Ok(result.clone())
        );
    }
}
