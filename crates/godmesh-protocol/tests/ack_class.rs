//! ADR-058 — taxonomia de ACK por fronteira; SPEC-001 §4.2, SPEC-003, SPEC-004 §4.
//! Regra sob teste: uma classe superior nunca é inferida de sucesso de camada inferior,
//! de tempo decorrido ou de ausência de erro.

use godmesh_protocol::ack::{AckClass, AckEvidence, AckError};

#[test]
fn parses_every_declared_class_and_rejects_unknown() {
    for (texto, esperado) in [
        ("transport_accepted", AckClass::TransportAccepted),
        ("persisted_local", AckClass::PersistedLocal),
        ("replicated_to_authority", AckClass::ReplicatedToAuthority),
        ("materialized", AckClass::Materialized),
        ("delivered_to_device", AckClass::DeliveredToDevice),
        ("acknowledged_human", AckClass::AcknowledgedHuman),
    ] {
        assert_eq!(AckClass::parse(texto), Ok(esperado), "classe {texto}");
    }

    assert_eq!(AckClass::parse("delivered"), Err(AckError::UnknownClass));
    assert_eq!(AckClass::parse(""), Err(AckError::MissingClass));
}

#[test]
fn claim_above_available_evidence_is_refused() {
    // Evidência local de persistência não autoriza alegar entrega a dispositivo.
    let erro = AckEvidence::new(AckClass::PersistedLocal, "inbox:tenant-1:stream-9")
        .claim(AckClass::DeliveredToDevice)
        .unwrap_err();

    assert_eq!(erro, AckError::PromotionRefused { from: AckClass::PersistedLocal, to: AckClass::DeliveredToDevice });
}

#[test]
fn claim_at_or_below_evidence_is_allowed() {
    let ev = AckEvidence::new(AckClass::Materialized, "projection:tenant-1");
    assert!(ev.claim(AckClass::Materialized).is_ok());
    assert!(ev.claim(AckClass::PersistedLocal).is_ok());
}

#[test]
fn elapsed_time_is_not_evidence() {
    // Não existe API que promova por tempo: a única entrada é evidência explícita.
    let ev = AckEvidence::new(AckClass::TransportAccepted, "http:202");
    for classe in [AckClass::PersistedLocal, AckClass::Materialized, AckClass::DeliveredToDevice, AckClass::AcknowledgedHuman] {
        assert!(ev.claim(classe).is_err(), "não pode promover para {classe:?}");
    }
}
