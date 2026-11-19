use std::num::NonZeroU32;

use godmesh_federation::relay_wire::{
    Accept, Control, ErrorCode, Open, Refuse, WireError, decode_control, encode_control,
    frame_length, validate_reply,
};

const CONTROL_LIMIT: NonZeroU32 = NonZeroU32::new(512).unwrap();
const DATAGRAM_LIMIT: NonZeroU32 = NonZeroU32::new(1400).unwrap();
const REQUEST_ID: &str = "0123456789abcdef0123456789abcdef";
const TOKEN: &str = "000102030405060708090a0b0c0d0e0f";

fn request() -> Open {
    Open {
        request_id: REQUEST_ID.into(),
        route_id: "route_D1-01".into(),
    }
}

fn acceptance() -> Accept {
    let open = request();
    Accept {
        request_id: open.request_id,
        route_id: open.route_id,
        max_datagram_bytes: DATAGRAM_LIMIT,
        ready_token: TOKEN.into(),
    }
}

#[test]
fn controles_conferem_schema_e_bytes_do_token() {
    let request = request();
    let accept = acceptance();
    let open = Control::Open(request.clone());
    let reply = Control::Accept(accept.clone());

    for (frame, expected) in [
        (
            open,
            format!(
                r#"{{"type":"m5.relay.open.v1","request_id":"{REQUEST_ID}","route_id":"route_D1-01"}}"#
            ),
        ),
        (
            reply,
            format!(
                r#"{{"type":"m5.relay.accept.v1","request_id":"{REQUEST_ID}","route_id":"route_D1-01","max_datagram_bytes":1400,"ready_token":"{TOKEN}"}}"#
            ),
        ),
    ] {
        let bytes = encode_control(&frame, CONTROL_LIMIT).unwrap();
        assert_eq!(bytes, expected.as_bytes());
        assert_eq!(decode_control(&bytes, CONTROL_LIMIT).unwrap(), frame);
    }
    assert_eq!(
        accept.token_bytes().unwrap(),
        std::array::from_fn(|i| i as u8)
    );
    assert_eq!(
        validate_reply(&request, &Control::Accept(accept), DATAGRAM_LIMIT),
        Ok(())
    );
}

#[test]
fn recusa_tokens_e_campos_fora_do_schema_estrito() {
    let invalid = [
        format!(
            r#"{{"type":"m5.relay.open.v1","request_id":"{REQUEST_ID}","route_id":"d","addr":"127.0.0.1"}}"#
        ),
        format!(
            r#"{{"type":"m5.relay.open.v1","request_id":"{REQUEST_ID}","route_id":"d","route_id":"e"}}"#
        ),
        format!(
            r#"{{"type":"m5.relay.open.v1","type":"m5.relay.open.v1","request_id":"{REQUEST_ID}","route_id":"d"}}"#
        ),
        format!(r#"{{"type":"m5.relay.open.v1","request_id":"{REQUEST_ID}"}}"#),
        format!(r#"["m5.relay.open.v1","{REQUEST_ID}","d"]"#),
        format!(r#"{{"type":"m5.relay.open.v1","request_id":"{REQUEST_ID}","route_id":"d"}} {{}}"#),
        format!(
            r#"{{"type":"m5.relay.refuse.v1","request_id":"{REQUEST_ID}","route_id":"d","error":"unknown"}}"#
        ),
        format!(
            r#"{{"type":"m5.relay.accept.v1","request_id":"{REQUEST_ID}","route_id":"d","max_datagram_bytes":0,"ready_token":"{TOKEN}"}}"#
        ),
        format!(
            r#"{{"type":"m5.relay.accept.v1","request_id":"{REQUEST_ID}","route_id":"d","max_datagram_bytes":4294967296,"ready_token":"{TOKEN}"}}"#
        ),
    ];
    for body in invalid {
        assert_eq!(
            decode_control(body.as_bytes(), CONTROL_LIMIT),
            Err(WireError::MalformedControl),
            "{body}"
        );
    }

    let mut invalid_accept = acceptance();
    invalid_accept.ready_token = TOKEN.to_uppercase();
    assert_eq!(
        invalid_accept.token_bytes(),
        Err(WireError::InvalidField("ready_token"))
    );
    assert_eq!(
        encode_control(&Control::Accept(invalid_accept), CONTROL_LIMIT),
        Err(WireError::InvalidField("ready_token"))
    );
    let mut invalid_request = request();
    invalid_request.route_id = "é".repeat(32);
    assert_eq!(
        encode_control(&Control::Open(invalid_request), CONTROL_LIMIT),
        Err(WireError::InvalidField("route_id"))
    );
    let mut invalid_request = request();
    invalid_request.request_id.pop();
    assert_eq!(
        encode_control(&Control::Open(invalid_request), CONTROL_LIMIT),
        Err(WireError::InvalidField("request_id"))
    );
}

#[test]
fn resposta_exige_tipo_eco_e_teto_admitido() {
    let open = request();
    assert_eq!(
        validate_reply(&open, &Control::Open(open.clone()), DATAGRAM_LIMIT),
        Err(WireError::UnexpectedControl)
    );
    let mut reply = acceptance();
    reply.max_datagram_bytes = NonZeroU32::new(1401).unwrap();
    assert_eq!(
        validate_reply(&open, &Control::Accept(reply), DATAGRAM_LIMIT),
        Err(WireError::InvalidField("max_datagram_bytes"))
    );
    let mut reply = acceptance();
    reply.route_id.push('2');
    assert_eq!(
        validate_reply(&open, &Control::Accept(reply), DATAGRAM_LIMIT),
        Err(WireError::CorrelationMismatch)
    );

    for (error, code) in [
        (ErrorCode::RouteDenied, "route_denied"),
        (ErrorCode::AuthorityBlocked, "authority_blocked"),
        (ErrorCode::QuotaExceeded, "quota_exceeded"),
        (ErrorCode::RouteUnavailable, "route_unavailable"),
    ] {
        let mut refuse = Refuse {
            request_id: open.request_id.clone(),
            route_id: open.route_id.clone(),
            error,
        };
        let bytes = encode_control(&Control::Refuse(refuse.clone()), CONTROL_LIMIT).unwrap();
        let json = format!(
            r#"{{"type":"m5.relay.refuse.v1","request_id":"{REQUEST_ID}","route_id":"route_D1-01","error":"{code}"}}"#
        );
        assert_eq!(bytes, json.as_bytes());
        let decoded = decode_control(&bytes, CONTROL_LIMIT).unwrap();
        assert_eq!(decoded, Control::Refuse(refuse.clone()));
        assert_eq!(validate_reply(&open, &decoded, DATAGRAM_LIMIT), Ok(()));
        refuse.request_id = "e".repeat(32);
        assert_eq!(
            validate_reply(&open, &Control::Refuse(refuse), DATAGRAM_LIMIT),
            Err(WireError::CorrelationMismatch)
        );
    }
}

#[test]
fn comprimento_e_limitado_antes_do_corpo() {
    assert_eq!(
        frame_length(0_u32.to_be_bytes(), CONTROL_LIMIT),
        Err(WireError::InvalidLength)
    );
    assert_eq!(
        frame_length(512_u32.to_be_bytes(), CONTROL_LIMIT),
        Ok(CONTROL_LIMIT)
    );
    assert_eq!(
        frame_length(513_u32.to_be_bytes(), CONTROL_LIMIT),
        Err(WireError::LimitExceeded)
    );
    assert_eq!(
        frame_length(1400_u32.to_be_bytes(), DATAGRAM_LIMIT),
        Ok(DATAGRAM_LIMIT)
    );
    assert_eq!(
        frame_length(1401_u32.to_be_bytes(), DATAGRAM_LIMIT),
        Err(WireError::LimitExceeded)
    );

    let mut bytes = encode_control(&Control::Open(request()), CONTROL_LIMIT).unwrap();
    bytes.resize(512, b' ');
    assert_eq!(
        decode_control(&bytes, CONTROL_LIMIT).unwrap(),
        Control::Open(request())
    );
    bytes.push(b' ');
    assert_eq!(
        decode_control(&bytes, CONTROL_LIMIT),
        Err(WireError::LimitExceeded)
    );
}
