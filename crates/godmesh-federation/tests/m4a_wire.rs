use std::num::{NonZeroU32, NonZeroUsize};

use godmesh_federation::m4a_wire::{
    Ack, AckClass, Batch, ErrorCode, ErrorMessage, Limits, Message, Namespace, Pull, WireError,
    decode_body, encode_body, validate_ack, validate_batch, validate_error,
};
use serde_json::{Value, json};

fn limits(frame: u32, envelope: usize, events: usize) -> Limits {
    Limits {
        frame_bytes: NonZeroU32::new(frame).unwrap(),
        envelope_bytes: NonZeroUsize::new(envelope).unwrap(),
        event_count: NonZeroUsize::new(events).unwrap(),
    }
}

fn pull() -> Pull {
    Pull {
        request_id: "0123456789abcdef0123456789abcdef".into(),
        namespace: Namespace {
            origin_node: format!("gmn_{}", "a".repeat(64)),
            tenant_id: "tenant".into(),
            stream_id: "m4a.metadata.test".into(),
            partition: 0,
            authority_epoch: 1,
        },
        destination: format!("gmn_{}", "b".repeat(64)),
        after_seq: 0,
        receiver_limits: limits(12000, 2000, 2),
    }
}

fn event(seq: i64) -> Vec<u8> {
    let node = pull().namespace.origin_node;
    format!(
        concat!(
            " \t\r\n{{\"position\":{{\"origin_node\":\"{}\",\"stream_id\":",
            "\"m4a.metadata.test\",\"partition\":0,\"authority_epoch\":1,\"seq\":{}}},",
            "\"tenant_id\":\"te\\u006eant\",\"retention_class\":\"a,]{{\\\"\\\\[}}\",",
            "\"issued_at\":-9223372036854775808,\"node_id\":\"{}\",\"signature\":\"{}\"}}\n \t"
        ),
        node,
        seq,
        node,
        "1".repeat(128)
    )
    .into_bytes()
}

fn batch<'a>(events: Vec<&'a [u8]>) -> Batch<'a> {
    let pull = pull();
    let source_limits = limits(10000, 3000, 3);
    Batch {
        request_id: pull.request_id,
        namespace: pull.namespace,
        destination: pull.destination,
        after_seq: pull.after_seq,
        source_limits,
        effective_limits: source_limits.component_min(pull.receiver_limits),
        has_more: !events.is_empty(),
        events,
    }
}

fn ceiling() -> NonZeroU32 {
    limits(16000, 1, 1).frame_bytes
}

fn decode(bytes: &[u8]) -> Result<Message<'_>, WireError> {
    decode_body(bytes, ceiling())
}

fn encode(message: &Message<'_>) -> Vec<u8> {
    encode_body(message, ceiling()).unwrap()
}

#[test]
fn p_wire_names_and_control_round_trips() {
    let pull = pull();
    let ack = Ack {
        request_id: pull.request_id.clone(),
        namespace: pull.namespace.clone(),
        destination: pull.destination.clone(),
        through_seq: 1,
        ack_class: AckClass::PersistedLocal,
    };
    let messages = [
        (Message::Pull(pull.clone()), "m4a.pull.v1"),
        (Message::Ack(ack.clone()), "m4a.ack.v1"),
    ];
    for (message, kind) in messages {
        let bytes = encode(&message);
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["type"], kind);
        assert_eq!(decode(&bytes).unwrap(), message);
    }
    assert!(validate_ack(&pull, &ack, 1).is_ok());
    for (error, code) in [
        (ErrorCode::ScopeDenied, "scope_denied"),
        (ErrorCode::ResidenceDenied, "residence_denied"),
        (ErrorCode::AuthorityBlocked, "authority_blocked"),
        (ErrorCode::ProofMissing, "proof_missing"),
        (ErrorCode::ProofConflict, "proof_conflict"),
        (ErrorCode::SequenceGap, "sequence_gap"),
        (ErrorCode::HistoryUnavailable, "history_unavailable"),
        (ErrorCode::LimitExceeded, "limit_exceeded"),
        (ErrorCode::StorageUnavailable, "storage_unavailable"),
    ] {
        let error = ErrorMessage {
            request_id: pull.request_id.clone(),
            error,
        };
        assert!(validate_error(&pull, &error).is_ok());
        let message = Message::Error(error);
        let bytes = encode(&message);
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            json,
            json!({"type":"m4a.error.v1", "request_id":pull.request_id, "error":code})
        );
        assert_eq!(decode(&bytes).unwrap(), message);
    }
}

#[test]
fn p_batch_borrows_original_delimited_bytes() {
    let first = event(1);
    let second = event(2);
    let batch = batch(vec![&first, &second]);
    let bytes = encode(&Message::Batch(batch.clone()));
    let Message::Batch(received) = decode(&bytes).unwrap() else {
        panic!("expected Batch")
    };
    assert_eq!(received, batch);
    assert!(validate_batch(&pull(), &received, bytes.len()).is_ok());
    let mut array = vec![b'['];
    array.extend(&first);
    array.push(b',');
    array.extend(&second);
    array.push(b']');
    assert!(bytes.windows(array.len()).any(|window| window == array));
    for envelope in received.events {
        let offset = envelope.as_ptr() as usize - bytes.as_ptr() as usize;
        assert_eq!(&bytes[offset..offset + envelope.len()], envelope);
    }
}

#[test]
fn n_strict_wire_and_event_shapes() {
    let pull_message = encode(&Message::Pull(pull()));
    let pull_json = String::from_utf8(pull_message).unwrap();
    let original: Value = serde_json::from_str(&pull_json).unwrap();
    for (field, value) in [
        ("type", json!("m4a.future.v1")),
        ("request_id", json!("F".repeat(32))),
        ("destination", json!("short")),
        ("after_seq", json!(-1)),
        ("after_seq", json!(0.0)),
        ("after_seq", json!(u64::MAX)),
        ("receiver_limits", json!([1, 2, 3])),
        (
            "receiver_limits",
            json!({"frame_bytes":4294967296_u64,"envelope_bytes":1,"event_count":1}),
        ),
        (
            "receiver_limits",
            json!({"frame_bytes":1,"envelope_bytes":1,"event_count":0}),
        ),
        (
            "namespace",
            json!([pull().namespace.origin_node, "tenant", "stream", 0, 1]),
        ),
        ("extra", json!(null)),
    ] {
        let mut bad = original.clone();
        bad[field] = value;
        assert!(
            decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
            "{field}: {bad}"
        );
    }
    for (needle, replacement) in [
        ("\"type\":", "\"type\":\"m4a.pull.v1\",\"type\":"),
        ("\"after_seq\":0,", ""),
        ("\"partition\":0", "\"partition\":-1"),
        ("\"authority_epoch\":1", "\"authority_epoch\":0"),
        (
            "\"authority_epoch\":1",
            "\"authority_epoch\":1,\"authority_epoch\":1",
        ),
        ("\"event_count\":2", "\"event_count\":2,\"event_count\":2"),
    ] {
        assert!(pull_json.contains(needle));
        assert!(decode(pull_json.replacen(needle, replacement, 1).as_bytes()).is_err());
    }

    let good_event = event(1);
    let json = String::from_utf8(encode(&Message::Batch(batch(vec![&good_event])))).unwrap();
    for (needle, replacement) in [
        ("\"seq\":1", "\"seq\":0"),
        ("\"seq\":1", "\"seq\":18446744073709551615"),
        ("\"seq\":1", "\"seq\":1,\"seq\":1"),
        ("\"tenant_id\":\"te", "\"payload\":{},\"tenant_id\":\"te"),
        ("\"position\":{", "\"position\":[{"),
    ] {
        assert!(json.contains(needle));
        assert!(
            decode(json.replacen(needle, replacement, 1).as_bytes()).is_err(),
            "{replacement}"
        );
    }
    let mut bad: Value = serde_json::from_str(&json).unwrap();
    bad["events"][0]["position"] =
        json!([pull().namespace.origin_node, "m4a.metadata.test", 0, 1, 1]);
    assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
    bad["events"] = json!([["pos", "tenant", "class", 0, "node", "sig"]]);
    assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
}

#[test]
fn b_limits_use_actual_json_and_original_whitespace() {
    let event = event(1);
    let mut sent = batch(vec![&event]);
    let body = encode(&Message::Batch(sent.clone()));
    let exact = NonZeroU32::new(body.len().try_into().unwrap()).unwrap();
    assert!(decode_body(&body, exact).is_ok());
    assert_eq!(
        decode_body(&body, NonZeroU32::new(exact.get() - 1).unwrap()),
        Err(WireError::LimitExceeded)
    );
    assert_eq!(
        encode_body(&Message::Batch(sent.clone()), exact).unwrap(),
        body
    );
    assert_eq!(
        encode_body(
            &Message::Batch(sent.clone()),
            NonZeroU32::new(exact.get() - 1).unwrap()
        ),
        Err(WireError::LimitExceeded)
    );
    let mut pull = pull();
    pull.receiver_limits.frame_bytes = exact;
    sent.effective_limits.frame_bytes = exact;
    assert!(validate_batch(&pull, &sent, body.len()).is_ok());
    assert_eq!(
        validate_batch(&pull, &sent, body.len() + 1),
        Err(WireError::LimitExceeded)
    );

    pull.receiver_limits.envelope_bytes = NonZeroUsize::new(event.len()).unwrap();
    sent.effective_limits.envelope_bytes = pull.receiver_limits.envelope_bytes;
    assert!(validate_batch(&pull, &sent, body.len()).is_ok());
    pull.receiver_limits.envelope_bytes = NonZeroUsize::new(event.len() - 1).unwrap();
    sent.effective_limits.envelope_bytes = pull.receiver_limits.envelope_bytes;
    assert_eq!(
        validate_batch(&pull, &sent, body.len()),
        Err(WireError::LimitExceeded)
    );
    assert_eq!(
        encode_body(&Message::Batch(sent.clone()), ceiling()),
        Err(WireError::LimitExceeded)
    );
    sent.effective_limits.event_count = NonZeroUsize::new(1).unwrap();
    assert_eq!(
        validate_batch(&pull, &sent, body.len()),
        Err(WireError::LimitsMismatch)
    );
}

#[test]
fn n_correlation_and_count() {
    let pull = pull();
    let envelope = event(1);
    let mut sent = batch(vec![&envelope]);
    let body_len = encode(&Message::Batch(sent.clone())).len();
    for field in 0..4 {
        let mut wrong = sent.clone();
        match field {
            0 => wrong.request_id = "1".repeat(32),
            1 => wrong.namespace.authority_epoch += 1,
            2 => wrong.destination = wrong.namespace.origin_node.clone(),
            3 => wrong.after_seq += 1,
            _ => unreachable!(),
        }
        assert_eq!(
            validate_batch(&pull, &wrong, body_len),
            Err(WireError::CorrelationMismatch)
        );
    }
    sent.events = vec![&envelope; 3];
    assert_eq!(
        validate_batch(&pull, &sent, body_len),
        Err(WireError::LimitExceeded)
    );
    let error = ErrorMessage {
        request_id: "1".repeat(32),
        error: ErrorCode::HistoryUnavailable,
    };
    assert_eq!(
        validate_error(&pull, &error),
        Err(WireError::CorrelationMismatch)
    );
    let ack = Ack {
        request_id: pull.request_id.clone(),
        namespace: pull.namespace.clone(),
        destination: pull.destination.clone(),
        ack_class: AckClass::PersistedLocal,
        through_seq: 2,
    };
    assert_eq!(
        validate_ack(&pull, &ack, 1),
        Err(WireError::CorrelationMismatch)
    );
}

#[test]
fn b_empty_batch_and_persistable_extremes() {
    let mut empty = batch(vec![]);
    let body = encode(&Message::Batch(empty.clone()));
    assert!(String::from_utf8_lossy(&body).contains("\"events\":[]"));
    assert_eq!(decode(&body), Ok(Message::Batch(empty.clone())));
    empty.has_more = true;
    assert!(encode_body(&Message::Batch(empty), ceiling()).is_err());
    assert!(
        decode(
            &String::from_utf8(body)
                .unwrap()
                .replace("\"has_more\":false", "\"has_more\":true")
                .into_bytes()
        )
        .is_err()
    );
    let mut pull = pull();
    pull.namespace.authority_epoch = i64::MAX;
    pull.after_seq = i64::MAX;
    let expected = Message::Pull(pull);
    assert_eq!(decode(&encode(&expected)), Ok(expected));
    let mut nonempty = batch(vec![]);
    let envelope = event(i64::MAX);
    nonempty.events.push(&envelope);
    assert!(decode(&encode(&Message::Batch(nonempty))).is_ok());
}

#[test]
fn f_invalid_frames_do_not_affect_following_message() {
    let expected = Message::Pull(pull());
    let good = encode(&expected);
    let mut extra = good.clone();
    extra.extend_from_slice(b" {}");
    for frame in [
        &[][..],
        &good[..good.len() - 1],
        &extra,
        b"[{}]",
        b"{\"invalid\":\"\xff\"}",
    ] {
        assert!(decode(frame).is_err());
        assert_eq!(decode(&good), Ok(expected.clone()));
    }
}
