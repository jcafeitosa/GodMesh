use std::num::{NonZeroU32, NonZeroUsize};

use godmesh_federation::{
    snapshot::{SnapshotIdentity, SnapshotLimits, SnapshotPolicy, verify_snapshot},
    snapshot_wire::{
        Ack, AckClass, Control, ErrorCode, ErrorMessage, Namespace, Request, Response, WireError,
        decode_control, encode_control, validate_ack, validate_error, validate_manifest,
        validate_package, validate_response,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn nz(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value).unwrap()
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/m4-b1-vetor.json")).unwrap()
}

fn package() -> Vec<u8> {
    hex::decode(fixture()["package"].as_str().unwrap()).unwrap()
}

fn request() -> Request {
    let fixture = fixture();
    let manifest = &fixture["manifest"];
    let text = |field: &str| manifest[field].as_str().unwrap().to_owned();
    Request {
        request_id: "0123456789abcdef0123456789abcdef".into(),
        origin_node: text("origin_node"),
        destination_node: text("destination_node"),
        namespace: Namespace {
            cluster: text("cluster"),
            tenant_id: text("tenant_id"),
            stream_id: text("stream_id"),
            partition: 0,
            authority_epoch: 7,
        },
        base_seq: 0,
        receiver_max_snapshot_bytes: nz(4096),
    }
}

fn response(request: &Request) -> Response {
    let package = package();
    Response {
        request_id: request.request_id.clone(),
        origin_node: request.origin_node.clone(),
        destination_node: request.destination_node.clone(),
        namespace: request.namespace.clone(),
        base_seq: request.base_seq,
        cut_seq: 2,
        package_bytes: nz(package.len().try_into().unwrap()),
        source_max_snapshot_bytes: nz(8192),
        package_sha256: hex::encode(Sha256::digest(package)),
    }
}

fn ack(response: &Response) -> Ack {
    Ack {
        request_id: response.request_id.clone(),
        origin_node: response.origin_node.clone(),
        destination_node: response.destination_node.clone(),
        namespace: response.namespace.clone(),
        cut_seq: response.cut_seq,
        package_sha256: response.package_sha256.clone(),
        ack_class: AckClass::PersistedLocal,
    }
}

fn error(request: &Request, error: ErrorCode) -> ErrorMessage {
    ErrorMessage {
        request_id: request.request_id.clone(),
        origin_node: request.origin_node.clone(),
        destination_node: request.destination_node.clone(),
        namespace: request.namespace.clone(),
        error,
    }
}

fn encode(control: &Control) -> Vec<u8> {
    encode_control(control, nz(16384)).unwrap()
}

fn decode(frame: &[u8]) -> Result<Control, WireError> {
    decode_control(frame, nz(16384))
}

#[test]
fn p_controls_and_all_stable_error_codes() {
    let request = request();
    let response = response(&request);
    let ack = ack(&response);
    for (control, kind) in [
        (Control::Request(request.clone()), "m4b.snapshot.request.v1"),
        (
            Control::Response(response.clone()),
            "m4b.snapshot.response.v1",
        ),
        (Control::Ack(ack.clone()), "m4b.snapshot.ack.v1"),
    ] {
        let frame = encode(&control);
        let v: Value = serde_json::from_slice(&frame).unwrap();
        assert_eq!(v["type"], kind);
        assert_eq!(v["namespace"]["cluster"], request.namespace.cluster);
        assert_eq!(decode(&frame), Ok(control));
        if kind.ends_with("ack.v1") {
            assert_eq!(v["ack_class"], "persisted_local");
        }
    }

    for (kind, code) in [
        ("scope_denied", ErrorCode::ScopeDenied),
        ("authority_blocked", ErrorCode::AuthorityBlocked),
        ("residence_denied", ErrorCode::ResidenceDenied),
        ("history_unavailable", ErrorCode::HistoryUnavailable),
        ("sequence_gap", ErrorCode::SequenceGap),
        ("proof_missing", ErrorCode::ProofMissing),
        ("proof_conflict", ErrorCode::ProofConflict),
        ("limit_exceeded", ErrorCode::LimitExceeded),
        ("snapshot_conflict", ErrorCode::SnapshotConflict),
        ("storage_unavailable", ErrorCode::StorageUnavailable),
    ] {
        let error = error(&request, code);
        let control = Control::Error(error.clone());
        let frame = encode(&control);
        let v: Value = serde_json::from_slice(&frame).unwrap();
        assert_eq!(
            v,
            json!({"type":"m4b.snapshot.error.v1", "request_id":request.request_id,
            "origin_node":request.origin_node, "destination_node":request.destination_node,
            "namespace":request.namespace, "error":kind})
        );
        assert_eq!(decode(&frame), Ok(control));
        assert_eq!(validate_error(&request, &error), Ok(()));
    }
    assert_eq!(validate_ack(&response, &ack), Ok(()));
}

#[test]
fn n_receive_strict_objects_types_and_ranges() {
    let request = request();
    let frame = encode(&Control::Request(request.clone()));
    let json: Value = serde_json::from_slice(&frame).unwrap();
    for (field, value) in [
        ("type", json!("m4b.snapshot.request.v2")),
        ("type", json!("m4a.pull.v1")),
        ("namespace", json!(["c", "t", "s", 0, 1])),
        ("base_seq", json!(0.0)),
        ("base_seq", json!(u64::MAX)),
        ("base_seq", json!(-1)),
        ("receiver_max_snapshot_bytes", json!(0)),
        ("receiver_max_snapshot_bytes", json!(4294967296_u64)),
        ("origin_node", json!(request.destination_node)),
        ("request_id", json!("F".repeat(32))),
        ("package_bytes", json!(1)),
    ] {
        let mut invalid = json.clone();
        invalid[field] = value;
        assert!(
            decode(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "{field}"
        );
    }
    let wire = String::from_utf8(frame).unwrap();
    for (needle, replacement) in [
        ("\"base_seq\":0", "\"base_seq\":0e0"),
        ("\"base_seq\":0", "\"base_seq\":-0"),
        ("\"base_seq\":0", "\"base_seq\":0,\"base_seq\":0"),
        (
            "\"type\":",
            "\"type\":\"m4b.snapshot.request.v1\",\"type\":",
        ),
        ("\"partition\":0", "\"partition\":0,\"partition\":0"),
        ("\"partition\":0", "\"partition\":0,\"extra\":0"),
        ("\"authority_epoch\":7", "\"authority_epoch\":0"),
    ] {
        assert!(wire.contains(needle));
        assert!(decode(wire.replacen(needle, replacement, 1).as_bytes()).is_err());
    }
    let mut missing = json;
    missing.as_object_mut().unwrap().remove("namespace");
    assert!(decode(&serde_json::to_vec(&missing).unwrap()).is_err());

    let response = response(&request);
    for (control, field, value) in [
        (Control::Ack(ack(&response)), "ack_class", json!("accepted")),
        (
            Control::Error(error(&request, ErrorCode::ScopeDenied)),
            "error",
            json!("snapshot_conflict "),
        ),
        (
            Control::Error(error(&request, ErrorCode::ScopeDenied)),
            "package_sha256",
            json!("0".repeat(64)),
        ),
        (Control::Response(response.clone()), "cut_seq", json!(0)),
        (
            Control::Response(response),
            "package_sha256",
            json!("A".repeat(64)),
        ),
    ] {
        let mut invalid: Value = serde_json::from_slice(&encode(&control)).unwrap();
        invalid[field] = value;
        assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
}

#[test]
fn n_encoder_validates_typed_fields() {
    let request = request();
    for mut bad in [
        request.clone(),
        request.clone(),
        request.clone(),
        request.clone(),
    ]
    .into_iter()
    .enumerate()
    {
        match bad.0 {
            0 => bad.1.namespace.stream_id = "m4a.metadata. ".into(),
            1 => bad.1.namespace.authority_epoch = 0,
            2 => bad.1.namespace.partition = 1,
            3 => bad.1.namespace.cluster = "é".repeat(129),
            _ => unreachable!(),
        }
        assert!(encode_control(&Control::Request(bad.1), nz(16384)).is_err());
    }

    let response = response(&request);
    let mut invalid = response.clone();
    invalid.cut_seq = invalid.base_seq;
    assert!(encode_control(&Control::Response(invalid), nz(16384)).is_err());
    let mut invalid = response.clone();
    invalid.source_max_snapshot_bytes = nz(1);
    assert_eq!(
        encode_control(&Control::Response(invalid), nz(16384)),
        Err(WireError::LimitExceeded)
    );
    let mut invalid = ack(&response);
    invalid.package_sha256 = "A".repeat(64);
    assert!(encode_control(&Control::Ack(invalid), nz(16384)).is_err());
    let mut invalid = error(&request, ErrorCode::SnapshotConflict);
    invalid.destination_node.clone_from(&invalid.origin_node);
    assert!(encode_control(&Control::Error(invalid), nz(16384)).is_err());
}

#[test]
fn n_full_correlation_response_ack_and_error() {
    let request = request();
    let response = response(&request);
    let ack = ack(&response);
    let error = error(&request, ErrorCode::HistoryUnavailable);
    let variants = [
        ("request_id", json!("1".repeat(32))),
        ("origin_node", json!(format!("gmn_{}", "c".repeat(64)))),
        ("destination_node", json!(format!("gmn_{}", "d".repeat(64)))),
        ("namespace.cluster", json!("other-cluster")),
        ("namespace.tenant_id", json!("other-tenant")),
        ("namespace.stream_id", json!("m4a.metadata.other")),
        ("namespace.authority_epoch", json!(8)),
    ];
    for (field, value) in variants {
        for control in [
            Control::Response(response.clone()),
            Control::Ack(ack.clone()),
            Control::Error(error.clone()),
        ] {
            let mut v: Value = serde_json::from_slice(&encode(&control)).unwrap();
            if let Some(field) = field.strip_prefix("namespace.") {
                v["namespace"][field] = value.clone();
            } else {
                v[field] = value.clone();
            }
            let decoded = decode(&serde_json::to_vec(&v).unwrap()).unwrap();
            let result = match decoded {
                Control::Response(received) => {
                    validate_response(&request, &received, nz(4096)).map(|_| ())
                }
                Control::Ack(received) => validate_ack(&response, &received),
                Control::Error(received) => validate_error(&request, &received),
                Control::Request(_) => unreachable!(),
            };
            assert_eq!(result, Err(WireError::CorrelationMismatch), "{field}");
        }
    }
    let mut received = response.clone();
    received.base_seq += 1;
    assert_eq!(
        validate_response(&request, &received, nz(4096)),
        Err(WireError::CorrelationMismatch)
    );
    let mut received = ack.clone();
    received.cut_seq += 1;
    assert_eq!(
        validate_ack(&response, &received),
        Err(WireError::CorrelationMismatch)
    );
    let mut received = ack;
    received.package_sha256 = "0".repeat(64);
    assert_eq!(
        validate_ack(&response, &received),
        Err(WireError::CorrelationMismatch)
    );
}

#[test]
fn b_binary_exact_hash_size_and_b1_manifest() {
    let mut request = request();
    let mut response = response(&request);
    let mut package = package();
    let exact = response.package_bytes;
    assert_eq!(validate_response(&request, &response, exact), Ok(exact));
    assert_eq!(
        validate_package(&request, &response, &package, exact),
        Ok(())
    );
    request.receiver_max_snapshot_bytes = nz(8193);
    assert_eq!(
        validate_response(&request, &response, nz(8193)),
        Ok(nz(8192))
    );
    request.receiver_max_snapshot_bytes = exact;
    assert_eq!(validate_response(&request, &response, nz(8193)), Ok(exact));
    assert_eq!(
        validate_response(&request, &response, nz(exact.get() - 1)),
        Err(WireError::LimitExceeded)
    );
    request.receiver_max_snapshot_bytes = nz(exact.get() - 1);
    assert_eq!(
        validate_response(&request, &response, exact),
        Err(WireError::LimitExceeded)
    );
    request.receiver_max_snapshot_bytes = exact;
    assert_eq!(
        validate_package(&request, &response, &package[..package.len() - 1], exact),
        Err(WireError::LengthMismatch)
    );

    let fixture = fixture();
    let policy = SnapshotPolicy {
        cluster: request.namespace.cluster.clone(),
        origin: SnapshotIdentity {
            node_id: request.origin_node.clone(),
            public_key: fixture["origin_public_key"].as_str().unwrap().into(),
        },
        destination: SnapshotIdentity {
            node_id: request.destination_node.clone(),
            public_key: fixture["destination_public_key"].as_str().unwrap().into(),
        },
        tenant_id: request.namespace.tenant_id.clone(),
        stream_id: request.namespace.stream_id.clone(),
        partition: 0,
        authority_epoch: 7,
        retention_class: fixture["retention_class"].as_str().unwrap().into(),
        limits: SnapshotLimits {
            max_package_bytes: NonZeroUsize::new(package.len()).unwrap(),
            max_manifest_bytes: nz(609),
            max_event_bytes: nz(488),
            max_events: nz(2),
        },
    };
    let verified = verify_snapshot(&package, &policy).unwrap();
    assert_eq!(validate_manifest(&response, &verified), Ok(()));
    response.cut_seq += 1;
    assert_eq!(
        validate_manifest(&response, &verified),
        Err(WireError::CorrelationMismatch)
    );
    response.cut_seq -= 1;
    response.namespace.cluster = "other-cluster".into();
    assert_eq!(
        validate_manifest(&response, &verified),
        Err(WireError::CorrelationMismatch)
    );
    response
        .namespace
        .cluster
        .clone_from(&request.namespace.cluster);
    let whitespace = package.iter().position(|b| *b == b'\t').unwrap();
    package[whitespace] = b' ';
    assert_eq!(
        validate_package(&request, &response, &package, exact),
        Err(WireError::HashMismatch)
    );
}

#[test]
fn f_control_limits_truncation_and_recovery() {
    let mut request = request();
    request.base_seq = i64::MAX as u64;
    request.receiver_max_snapshot_bytes = NonZeroU32::MAX;
    let control = Control::Request(request);
    let good = encode(&control);
    let limit = nz(good.len().try_into().unwrap());
    let smaller = nz(limit.get() - 1);
    assert_eq!(decode_control(&good, limit), Ok(control.clone()));
    assert_eq!(encode_control(&control, limit), Ok(good.clone()));
    assert_eq!(
        decode_control(&good, smaller),
        Err(WireError::LimitExceeded)
    );
    assert_eq!(
        encode_control(&control, smaller),
        Err(WireError::LimitExceeded)
    );
    let mut extra = good.clone();
    extra.extend_from_slice(b"{}");
    for frame in [
        b"".as_slice(),
        &good[..good.len() - 1],
        b"[{}]",
        b"{\"x\":\"\xff\"}",
        &extra,
    ] {
        assert_eq!(decode(frame), Err(WireError::MalformedControl));
        assert_eq!(decode_control(&good, limit), Ok(control.clone()));
    }
}
