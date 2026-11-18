//! ADR-058 — composição de ACKs entre camadas: agregar nunca promove.
//! Regra sob teste: o coletor devolve a menor classe comum; uma camada atrasada
//! impede alegar mais do que ela prova.

use godmesh_protocol::ack::{AckClass, AckComposer, AckError, AckEvidence};

#[test]
fn composition_returns_lowest_class_across_layers() {
    let mut c = AckComposer::new();
    c.add(AckEvidence::new(AckClass::Materialized, "projection:1"));
    c.add(AckEvidence::new(AckClass::PersistedLocal, "inbox:1"));
    assert_eq!(c.composed(), AckClass::PersistedLocal);
}

#[test]
fn single_evidence_returns_its_own_class() {
    let mut c = AckComposer::new();
    c.add(AckEvidence::new(AckClass::TransportAccepted, "http:202"));
    assert_eq!(c.composed(), AckClass::TransportAccepted);
}

#[test]
fn empty_composition_has_no_claim() {
    let c = AckComposer::new();
    assert_eq!(c.try_composed(), Err(AckError::NoEvidence));
}

#[test]
fn claiming_above_composition_is_refused() {
    let mut c = AckComposer::new();
    c.add(AckEvidence::new(AckClass::PersistedLocal, "inbox:1"));
    assert_eq!(
        c.claim(AckClass::DeliveredToDevice),
        Err(AckError::PromotionRefused { from: AckClass::PersistedLocal, to: AckClass::DeliveredToDevice })
    );
}

#[test]
fn adding_weaker_layer_lowers_the_claim() {
    let mut c = AckComposer::new();
    c.add(AckEvidence::new(AckClass::AcknowledgedHuman, "resp:9"));
    assert_eq!(c.composed(), AckClass::AcknowledgedHuman);
    c.add(AckEvidence::new(AckClass::TransportAccepted, "bridge:leg-2"));
    assert_eq!(
        c.composed(),
        AckClass::TransportAccepted,
        "a perna de bridge rebaixa a alegação do conjunto"
    );
}
