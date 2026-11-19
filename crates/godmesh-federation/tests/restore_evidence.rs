use std::{fs, io::Write, num::NonZeroUsize, os::unix::fs::OpenOptionsExt};

use godmesh_federation::{
    NodeIdentity,
    restore_evidence::{
        AuthorityKind, BackupKind, ControlState, CopyAcl, CopyPolicy, ExpectedHighWater,
        ExpectedPromotion, Fence, IntentState, Reconciliation, RestoreError, RestoreHighWaterV1,
        RestoreKey, RestorePromotionV1, RevocationReference, VerifiedHighWater, proofs_digest,
        verify_high_water, verify_promotion,
    },
};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/restore-vetor.json")).unwrap()
}

fn unhex<const N: usize>(value: &Value) -> [u8; N] {
    hex::decode(value.as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap()
}

fn digest(name: &str) -> [u8; 32] {
    unhex(&fixture()[name]["sha256"])
}

fn wire(name: &str) -> Vec<u8> {
    serde_json::to_vec(&fixture()[name]["wire"]).unwrap()
}

fn max_bytes() -> NonZeroUsize {
    NonZeroUsize::new(16384).unwrap()
}

fn a() -> String {
    format!("gmn_{}", "a".repeat(64))
}

fn b() -> String {
    format!("gmn_{}", "b".repeat(64))
}

fn key() -> RestoreKey {
    RestoreKey {
        fixture_id: "11".repeat(16),
        cluster: "c".into(),
        local_node: b(),
        origin_node: a(),
        tenant_id: "t".into(),
        stream_id: "m4a.metadata.v".into(),
        partition: 0,
        authority_epoch: 7,
    }
}

fn expected_high<'a>(key: &'a RestoreKey, pin: &'a str) -> ExpectedHighWater<'a> {
    ExpectedHighWater {
        key,
        target_db_id: "target",
        fenced_generation: 1,
        target_generation: 2,
        high_water_sha256: digest("high_water"),
        fence_sha256: digest("fence"),
        harness_public_key: pin,
    }
}

fn high() -> VerifiedHighWater {
    let value = fixture();
    verify_high_water(
        &wire("high_water"),
        max_bytes(),
        &expected_high(&key(), value["harness_public_key"].as_str().unwrap()),
    )
    .unwrap()
}

fn fence() -> Fence {
    Fence {
        key: key(),
        fenced_generation: 1,
        cut_db_id: "cut".into(),
        target_db_id: "target".into(),
        old_login: "old".into(),
        admission_closed: true,
        nologin_confirmed: true,
        grants_removed: true,
        remaining_sessions: 0,
        unknown_effects: 0,
        fenced_at: 100,
    }
}

fn rec(high_water_sha256: [u8; 32]) -> Reconciliation {
    Reconciliation {
        key: key(),
        target_generation: 2,
        high_water_sha256,
        final_prefix: 0,
        proofs_sha256: digest("empty_proofs"),
        current_control_sha256: digest("control"),
        blocked_next_seq: 0,
        intent_state: IntentState::Absent,
        attempt_id: 0,
    }
}

fn copy_policy() -> CopyPolicy {
    CopyPolicy {
        policy_id: "p".into(),
        policy_version: 1,
        tenant_id: "t".into(),
        retention_class: "meta".into(),
        class_version: 1,
        jurisdiction: "BR".into(),
        copy_node: a(),
        copy_kind: "primary".into(),
        purpose: "replica_metadata".into(),
        trigger_event: "issued_at".into(),
        duration_seconds: 100,
        allow_discard: true,
        hold_mode: "explicit".into(),
        active: true,
    }
}

fn acl() -> CopyAcl {
    CopyAcl {
        origin_node: a(),
        tenant_id: "t".into(),
        stream_id: key().stream_id,
        partition: 0,
        authority_epoch: 7,
        destination: b(),
        direction: "export".into(),
        retention_class: "meta".into(),
        class_version: 1,
        jurisdiction: "BR".into(),
        copy_kind: "secondary".into(),
        policy_id: "s".into(),
        policy_version: 2,
        not_before: 0,
        expires_at: 200,
        revoked: false,
    }
}

fn revocation() -> RevocationReference {
    RevocationReference {
        cluster: "c".into(),
        root_node_id: a(),
        target_digest: [0x22; 32],
        revocation_digest: [0x33; 32],
        accepted_at: 50,
        receipt_signature: [0x44; 64],
    }
}

fn control() -> ControlState {
    ControlState {
        key: key(),
        generation: 2,
        root_node_id: a(),
        authority_kind: AuthorityKind::Genesis,
        authority_ref: [0x55; 32],
        origin_revocations: vec![digest("revocation")],
        destination_revocations: vec![],
        m3_epoch: 1,
        m3_version: 2,
        m3_hash: [0x66; 32],
        m3_floor: 1,
        primary_policy_sha256: digest("primary_policy"),
        export_secondary_policy_sha256: digest("export_policy"),
        import_secondary_policy_sha256: digest("import_policy"),
        export_acl_sha256: digest("export_acl"),
        import_acl_sha256: digest("import_acl"),
        blocked_next_seq: 0,
        intent_state: IntentState::Absent,
        attempt_id: 0,
        prefix: 0,
    }
}

fn assert_vector(label: &str, bytes: Vec<u8>, hash: [u8; 32]) {
    assert_eq!(hex::encode(bytes), fixture()[label]["canonical_hex"]);
    assert_eq!(hash, digest(label));
}

fn signer() -> NodeIdentity {
    let nonce = NodeIdentity::generate_in_memory().unwrap().public_key_hex();
    let path = std::env::temp_dir().join(format!(
        "godmesh-restore-test-{}-{nonce}",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .unwrap();
    writeln!(file, "{}", hex::encode([0x19; 32])).unwrap();
    drop(file);
    let signer = NodeIdentity::load(&path);
    fs::remove_file(path).unwrap();
    signer.unwrap()
}

#[test]
fn p_independent_auxiliary_tlv_and_hashes() {
    let mut policy = copy_policy();
    assert_vector(
        "primary_policy",
        policy.canonical_bytes().unwrap(),
        policy.digest().unwrap(),
    );
    policy.policy_id = "s".into();
    policy.copy_kind = "secondary".into();
    policy.copy_node = b();
    policy.policy_version = 2;
    assert_vector(
        "export_policy",
        policy.canonical_bytes().unwrap(),
        policy.digest().unwrap(),
    );
    policy.policy_version = 3;
    assert_vector(
        "import_policy",
        policy.canonical_bytes().unwrap(),
        policy.digest().unwrap(),
    );

    let mut acl = acl();
    assert_vector(
        "export_acl",
        acl.canonical_bytes().unwrap(),
        acl.digest().unwrap(),
    );
    acl.direction = "import".into();
    acl.policy_version = 3;
    assert_vector(
        "import_acl",
        acl.canonical_bytes().unwrap(),
        acl.digest().unwrap(),
    );

    let rev = revocation();
    assert_vector(
        "revocation",
        rev.canonical_bytes().unwrap(),
        rev.digest().unwrap(),
    );
    let control = control();
    assert_vector(
        "control",
        control.canonical_bytes().unwrap(),
        control.digest().unwrap(),
    );
    let fence = fence();
    assert_vector(
        "fence",
        fence.canonical_bytes().unwrap(),
        fence.digest().unwrap(),
    );
    let rec = rec([0x77; 32]);
    assert_vector(
        "reconciliation",
        rec.canonical_bytes().unwrap(),
        rec.digest().unwrap(),
    );
}

#[test]
fn p_independent_signed_high_water_and_promotion() {
    let verified = high();
    let mut backup = hex::decode(fixture()["backup_hex"].as_str().unwrap()).unwrap();
    assert_eq!(verified.verify_backup(&backup), Ok(()));
    assert_eq!(
        verified.verify_backup(&backup[1..]),
        Err(RestoreError::BindingMismatch)
    );
    backup[0] ^= 1;
    assert_eq!(
        verified.verify_backup(&backup),
        Err(RestoreError::BindingMismatch)
    );
    let mut mark = verified.high_water().clone();
    assert_vector(
        "high_water",
        mark.canonical_bytes().unwrap(),
        verified.digest(),
    );
    let key = signer();
    mark.signature = [0; 64];
    mark.sign(&key).unwrap();
    assert_eq!(
        mark.signature,
        unhex(&fixture()["high_water"]["wire"]["signature"])
    );
    let rec = rec(verified.digest());
    let expected = ExpectedPromotion {
        high_water: &verified,
        reconciliation: &rec,
    };
    let verified = verify_promotion(&wire("promotion"), max_bytes(), &expected, 110).unwrap();
    let mut promotion = verified.promotion().clone();
    assert_vector(
        "promotion",
        promotion.canonical_bytes().unwrap(),
        verified.digest(),
    );
    promotion.signature = [0; 64];
    promotion.sign(&key).unwrap();
    assert_eq!(
        promotion.signature,
        unhex(&fixture()["promotion"]["wire"]["signature"])
    );
    assert_eq!(
        serde_json::to_value(promotion).unwrap(),
        fixture()["promotion"]["wire"]
    );
    assert_eq!(
        serde_json::to_value(mark).unwrap(),
        fixture()["high_water"]["wire"]
    );
}

#[test]
fn b_ordered_proofs_and_empty_commitment() {
    assert_eq!(proofs_digest(0, []).unwrap(), digest("empty_proofs"));
    let f = fixture();
    let frames: Vec<Vec<u8>> = f["two_proofs"]["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|raw| hex::decode(raw.as_str().unwrap()).unwrap())
        .collect();
    let mut preimage = b"godmesh.federation.restore.proofs.v1".to_vec();
    preimage.extend_from_slice(&2_u64.to_be_bytes());
    for frame in &frames {
        preimage.extend_from_slice(&u32::try_from(frame.len()).unwrap().to_be_bytes());
        preimage.extend_from_slice(frame);
    }
    assert_vector(
        "two_proofs",
        preimage,
        proofs_digest(2, frames.iter().map(Vec::as_slice)).unwrap(),
    );
    assert_ne!(
        proofs_digest(2, frames.iter().rev().map(Vec::as_slice)).unwrap(),
        digest("two_proofs")
    );
    assert!(proofs_digest(3, frames.iter().map(Vec::as_slice)).is_err());
    assert!(proofs_digest(1, frames.iter().map(Vec::as_slice)).is_err());
    assert!(proofs_digest(1, [b"".as_slice()]).is_err());
    assert!(proofs_digest(u64::MAX, []).is_err());
}

#[test]
fn n_auxiliary_mutations_and_invariants() {
    let mut policy = copy_policy();
    let primary = policy.digest().unwrap();
    policy.duration_seconds += 1;
    assert_ne!(policy.digest().unwrap(), primary);
    let mut acl = acl();
    acl.direction = "import".into();
    assert_ne!(acl.digest().unwrap(), digest("export_acl"));
    let mut rev = revocation();
    rev.receipt_signature[0] ^= 1;
    assert_ne!(rev.digest().unwrap(), digest("revocation"));
    rev = revocation();
    rev.revocation_digest[0] ^= 1;
    assert_ne!(rev.digest().unwrap(), digest("revocation"));

    let mut control = control();
    control.origin_revocations.push([0x10; 32]);
    let two = control.digest().unwrap();
    control.origin_revocations.reverse();
    assert_eq!(control.digest().unwrap(), two);
    control.destination_revocations.push([0x10; 32]);
    assert_ne!(control.digest().unwrap(), two);
    control.destination_revocations.clear();
    control.generation += 1;
    assert_ne!(control.digest().unwrap(), two);
    let mut control = self::control();
    let original = control.digest().unwrap();
    control.export_secondary_policy_sha256[0] ^= 1;
    assert_ne!(control.digest().unwrap(), original);
    control = self::control();
    control.import_secondary_policy_sha256[0] ^= 1;
    assert_ne!(control.digest().unwrap(), original);
    control = self::control();
    std::mem::swap(
        &mut control.export_secondary_policy_sha256,
        &mut control.import_secondary_policy_sha256,
    );
    assert_ne!(control.digest().unwrap(), original);
    control = self::control();
    control.attempt_id = 1;
    assert!(control.digest().is_err());
    control.intent_state = IntentState::Pending;
    assert_ne!(control.digest().unwrap(), original);
    control.attempt_id = 0;
    assert!(control.digest().is_err());
    let mut fence = fence();
    fence.remaining_sessions = 1;
    assert!(fence.digest().is_err());
    let mut rec = rec([0x77; 32]);
    rec.current_control_sha256[0] ^= 1;
    assert_ne!(rec.digest().unwrap(), digest("reconciliation"));
}

#[test]
fn n_strict_wire_and_recovery() {
    let f = fixture();
    let key = key();
    let pin = f["harness_public_key"].as_str().unwrap();
    let expected = expected_high(&key, pin);
    let packet = wire("high_water");
    for (field, value) in [
        ("version", json!(2)),
        ("key", json!([1, 2, 3, 4, 5, 6, 7, 8])),
        ("backup_bytes", json!(1.0)),
        ("backup_bytes", json!(u64::MAX)),
        ("generation", json!(1)),
        ("backup_kind", json!("AtCut")),
        ("intent_state", json!("absent")),
        ("signature", json!("A".repeat(128))),
        ("fence_sha256", json!("00")),
    ] {
        let mut changed = f["high_water"]["wire"].clone();
        changed[field] = value;
        assert!(
            verify_high_water(
                &serde_json::to_vec(&changed).unwrap(),
                max_bytes(),
                &expected
            )
            .is_err(),
            "{field}"
        );
    }
    let raw = String::from_utf8(packet.clone()).unwrap();
    for (from, to) in [
        ("\"version\":1", "\"version\":1,\"version\":1"),
        ("\"partition\":0", "\"partition\":0,\"partition\":0"),
        ("\"partition\":0", "\"partition\":-0"),
        ("\"partition\":0", "\"partition\":0e0"),
        ("\"partition\":0,", ""),
    ] {
        assert!(raw.contains(from));
        assert_eq!(
            verify_high_water(raw.replacen(from, to, 1).as_bytes(), max_bytes(), &expected),
            Err(RestoreError::MalformedFrame)
        );
    }
    for broken in [
        &packet[..packet.len() - 1],
        b"[1]",
        b"{\"key\":\"\xff\"}",
        format!("{raw} {{}}").as_bytes(),
    ] {
        assert_eq!(
            verify_high_water(broken, max_bytes(), &expected),
            Err(RestoreError::MalformedFrame)
        );
        assert!(verify_high_water(&packet, max_bytes(), &expected).is_ok());
    }
    assert_eq!(
        verify_high_water(&packet, NonZeroUsize::new(packet.len()).unwrap(), &expected)
            .unwrap()
            .digest(),
        digest("high_water")
    );
    assert_eq!(
        verify_high_water(
            &packet,
            NonZeroUsize::new(packet.len() - 1).unwrap(),
            &expected
        ),
        Err(RestoreError::FrameTooLarge)
    );
}

#[test]
fn n_authentic_artifacts_still_require_external_binding_and_window() {
    let f = fixture();
    let pin = f["harness_public_key"].as_str().unwrap();
    let key = key();
    let expected = expected_high(&key, pin);

    let mut altered = high().high_water().clone();
    altered.target_generation += 1;
    assert!(altered.canonical_bytes().is_err());
    altered = high().high_water().clone();
    altered.backup_kind = BackupKind::BeforeCut;
    assert!(altered.canonical_bytes().is_err());
    altered = high().high_water().clone();
    altered.gap_proof_count += 1;
    assert!(altered.canonical_bytes().is_err());

    let signer = signer();
    let mut high_water: RestoreHighWaterV1 = high().high_water().clone();
    high_water.cut_at -= 1;
    high_water.sign(&signer).unwrap();
    assert_eq!(
        verify_high_water(
            &serde_json::to_vec(&high_water).unwrap(),
            max_bytes(),
            &expected
        ),
        Err(RestoreError::BindingMismatch)
    );
    let mut high_water = high().high_water().clone();
    high_water.signature[0] ^= 1;
    assert_eq!(
        verify_high_water(
            &serde_json::to_vec(&high_water).unwrap(),
            max_bytes(),
            &expected
        ),
        Err(RestoreError::InvalidSignature)
    );

    let high_water = high();
    let mut reconciliation = rec(high_water.digest());
    let expected = ExpectedPromotion {
        high_water: &high_water,
        reconciliation: &reconciliation,
    };
    let packet = wire("promotion");
    for now in [109, 120] {
        assert_eq!(
            verify_promotion(&packet, max_bytes(), &expected, now),
            Err(RestoreError::OutsideWindow)
        );
    }
    assert!(verify_promotion(&packet, max_bytes(), &expected, 119).is_ok());
    let mut promotion: RestorePromotionV1 = verify_promotion(&packet, max_bytes(), &expected, 110)
        .unwrap()
        .promotion()
        .clone();
    promotion.signature[0] ^= 1;
    assert_eq!(
        verify_promotion(
            &serde_json::to_vec(&promotion).unwrap(),
            max_bytes(),
            &expected,
            110
        ),
        Err(RestoreError::InvalidSignature)
    );
    reconciliation.current_control_sha256[0] ^= 1;
    assert_eq!(
        verify_promotion(
            &packet,
            max_bytes(),
            &ExpectedPromotion {
                high_water: &high_water,
                reconciliation: &reconciliation
            },
            110
        ),
        Err(RestoreError::BindingMismatch)
    );
    let mut local_pending = rec(high_water.digest());
    local_pending.intent_state = IntentState::Pending;
    local_pending.attempt_id = 1;
    assert_eq!(
        verify_promotion(
            &packet,
            max_bytes(),
            &ExpectedPromotion {
                high_water: &high_water,
                reconciliation: &local_pending
            },
            110
        ),
        Err(RestoreError::BlockedIntent)
    );

    let mut before_cut = high_water.high_water().clone();
    before_cut.backup_kind = BackupKind::BeforeCut;
    before_cut.cut_prefix = 2;
    before_cut.gap_proof_count = 2;
    before_cut.gap_proofs_sha256 = digest("two_proofs");
    before_cut.blocked_next_seq = 3;
    before_cut.intent_state = IntentState::Indeterminate;
    before_cut.sign(&signer).unwrap();
    let mut current = expected_high(&key, pin);
    current.high_water_sha256 = before_cut.digest().unwrap();
    let closed = verify_high_water(
        &serde_json::to_vec(&before_cut).unwrap(),
        max_bytes(),
        &current,
    )
    .unwrap();
    let mut on_target = rec(closed.digest());
    on_target.final_prefix = 2;
    on_target.proofs_sha256 = before_cut.gap_proofs_sha256;
    on_target.blocked_next_seq = 3;
    promotion.high_water_sha256 = closed.digest();
    promotion.reconciliation_sha256 = on_target.digest().unwrap();
    promotion.sign(&signer).unwrap();
    assert_eq!(
        verify_promotion(
            &serde_json::to_vec(&promotion).unwrap(),
            max_bytes(),
            &ExpectedPromotion {
                high_water: &closed,
                reconciliation: &on_target
            },
            110
        ),
        Err(RestoreError::BlockedIntent)
    );
}
