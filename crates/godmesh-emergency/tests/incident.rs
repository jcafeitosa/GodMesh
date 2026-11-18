//! ADR-058/064/065 e SPEC-006 — máquina de estados do incidente SOS.
//! Regras sob teste: estado só avança com evidência da camada; nada alega
//! "acionado" sem persistência; reconhecimento humano exige prova de pessoa;
//! resultado desconhecido nunca é sucesso.

use godmesh_emergency::incident::{
    HumanProof, Incident, IncidentError, IncidentState,
};

#[test]
fn new_incident_is_local_attempt_and_never_says_acionado() {
    let inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    assert_eq!(inc.state(), IncidentState::TriggeredLocally);
    assert_eq!(inc.ui_label(), "tentativa local, não persistida");
    assert!(!inc.state().claims_delivery());
}

#[test]
fn persistence_moves_state_and_yields_only_persisted_local_ack() {
    let mut inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    inc.persist("inbox:tenant-1:3-7-42", "policy:sos:br:12m").expect("persistência local");
    assert_eq!(inc.state(), IncidentState::PersistedLocal);
    assert_eq!(inc.ack_class(), "persisted_local");
    assert_ne!(inc.ui_label(), "acionado");
}

#[test]
fn cannot_jump_from_triggered_to_delivery_or_human() {
    let mut inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    assert_eq!(inc.mark_delivered("device-7"), Err(IncidentError::IllegalTransition));
    assert_eq!(inc.mark_acknowledged_human(HumanProof::person("resp-9", 1_700_000_000)), Err(IncidentError::IllegalTransition));
    assert_eq!(inc.state(), IncidentState::TriggeredLocally);
}

#[test]
fn human_acknowledgement_requires_person_proof() {
    // Isola a regra de prova: incidente já na etapa técnica, prova sem pessoa.
    let mut inc = securer_incident();
    inc.mark_technical_ack("node-a", 1_700_000_010).unwrap();
    assert_eq!(
        inc.mark_acknowledged_human(HumanProof::anonymous()),
        Err(IncidentError::MissingHumanProof)
    );
    assert_eq!(inc.state(), IncidentState::AcknowledgedTechnical);
}

#[test]
fn human_acknowledgement_after_technical_ack_moves_to_human() {
    let mut inc = securer_incident();
    inc.mark_technical_ack("node-a", 1_700_000_010).expect("ACK técnico");
    inc.mark_acknowledged_human(HumanProof::person("resp-9", 1_700_000_020)).expect("ACK humano");
    assert_eq!(inc.state(), IncidentState::AcknowledgedHuman);
    assert_eq!(inc.ack_class(), "acknowledged_human");
}

#[test]
fn persistence_failure_yields_not_persisted_and_says_nao_salvo() {
    let mut inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    inc.fail_persistence("retention_policy_missing");
    assert_eq!(inc.state(), IncidentState::NotPersisted);
    assert_eq!(inc.ui_label(), "não salvo / não enfileirado");
    assert!(!inc.state().claims_delivery());
}

#[test]
fn ack_trail_is_append_only_and_records_source() {
    let inc = securer_incident();
    let classes: Vec<&str> = inc.acks().iter().map(|a| a.class).collect();
    assert_eq!(classes, vec!["persisted_local", "replicated_to_authority"]);
    assert_eq!(inc.acks()[1].source, "node-a");
}

#[test]
fn unknown_outcome_is_not_success() {
    let mut inc = securer_incident();
    inc.mark_outcome_unknown("timeout_sem_recibo");
    assert_eq!(inc.state(), IncidentState::OutcomeUnknown);
    assert!(!inc.state().is_success());
    assert_eq!(inc.ack_class(), "replicated_to_authority");
}

#[test]
fn close_requires_human_ack_or_cancel() {
    let mut inc = securer_incident();
    assert_eq!(inc.close("encerrado pelo operador"), Err(IncidentError::IllegalTransition));
    inc.mark_technical_ack("node-a", 1_700_000_010).unwrap();
    inc.mark_acknowledged_human(HumanProof::person("resp-9", 1_700_000_020)).unwrap();
    inc.close("encerrado pelo operador").expect("encerramento após ACK humano");
    assert_eq!(inc.state(), IncidentState::Closed);
}

#[test]
fn absent_revocation_is_not_unknown_and_blocks_nothing_but_state_does_not_lie() {
    // O incidente não decide autorização; ele apenas não pode alegar o que não houve.
    let inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    assert_eq!(inc.retention_class(), "incidente_sos");
    assert_eq!(inc.state().ack_class_name(), "transport_accepted");
}

fn securer_incident() -> Incident {
    let mut inc = Incident::triggered("inc-2", "tenant-1", "principal-7");
    inc.persist("inbox:tenant-1:3-7-43", "policy:sos:br:12m").unwrap();
    inc.mark_replicated("node-a", 1_700_000_005).unwrap();
    inc
}
