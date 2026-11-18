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
    inc.persist("inbox:tenant-1:3-7-42", "policy:sos:br:12m", 1_700_000_000).expect("persistência local");
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
    inc.fail_persistence("retention_policy_missing").expect("falha de persistência declarada");
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
    inc.mark_outcome_unknown("timeout_sem_recibo").expect("enviado sem confirmação");
    assert_eq!(inc.state(), IncidentState::OutcomeUnknown);
    assert!(!inc.state().is_success());
    // Sem recibo da autoridade, a classe do estado não pode alegar replicação.
    assert_eq!(inc.ack_class(), "transport_accepted");
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
    inc.persist("inbox:tenant-1:3-7-43", "policy:sos:br:12m", 1_700_000_001).unwrap();
    inc.mark_replicated("node-a", 1_700_000_005).unwrap();
    inc
}

// --- Gate BC (18/11/2026): guarda de estado, classe de ACK honesta e instante real ---

#[test]
fn fail_persistence_from_triggered_locally_is_accepted() {
    let mut inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    inc.fail_persistence("retention_policy_missing").expect("só TriggeredLocally pode falhar a persistência");
    assert_eq!(inc.state(), IncidentState::NotPersisted);
}

#[test]
fn fail_persistence_from_closed_is_refused_and_state_is_unchanged() {
    let mut inc = closed_incident();
    let trilha_antes = inc.acks().to_vec();
    assert_eq!(inc.fail_persistence("retry_cego"), Err(IncidentError::IllegalTransition));
    assert_eq!(inc.state(), IncidentState::Closed, "trilha append-only não regride");
    assert_eq!(inc.acks(), trilha_antes.as_slice(), "nenhum ACK novo é criado por transição ilegal");
}

#[test]
fn fail_persistence_from_persisted_local_is_refused() {
    let mut inc = securer_incident();
    assert_eq!(inc.fail_persistence("retry_cego"), Err(IncidentError::IllegalTransition));
    assert_eq!(inc.state(), IncidentState::ReplicatedToAuthority, "estado com evidência não regride");
}

#[test]
fn outcome_unknown_from_closed_is_refused_and_state_is_unchanged() {
    let mut inc = closed_incident();
    assert_eq!(inc.mark_outcome_unknown("timeout_sem_recibo"), Err(IncidentError::IllegalTransition));
    assert_eq!(inc.state(), IncidentState::Closed);
}

#[test]
fn outcome_unknown_is_refused_before_persistence() {
    let mut inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    assert_eq!(inc.mark_outcome_unknown("timeout_sem_recibo"), Err(IncidentError::IllegalTransition));
    assert_eq!(inc.state(), IncidentState::TriggeredLocally);
}

#[test]
fn outcome_unknown_only_after_a_real_send() {
    for mut inc in [securer_incident(), {
        let mut i = Incident::triggered("inc-3", "tenant-1", "principal-8");
        i.persist("inbox:tenant-1:3-7-44", "policy:sos:br:12m", 1_700_000_001).unwrap();
        i
    }] {
        inc.mark_outcome_unknown("timeout_sem_recibo").expect("enviado sem confirmação");
        assert_eq!(inc.state(), IncidentState::OutcomeUnknown);
    }
}

#[test]
fn unknown_outcome_never_claims_it_was_replicated() {
    let mut inc = securer_incident();
    inc.mark_outcome_unknown("timeout_sem_recibo").unwrap();
    let classe = inc.state().ack_class_name();
    assert_ne!(classe, "replicated_to_authority", "sem recibo não há replicação provada");
    assert_eq!(classe, "transport_accepted", "classe do CHECK de sos_ack");
}

#[test]
fn persist_records_the_real_instant() {
    let mut inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    inc.persist("inbox:tenant-1:3-7-42", "policy:sos:br:12m", 1_700_000_123).expect("persistência local");
    let registro = inc.acks().first().expect("um ACK persistido");
    assert_eq!(registro.class, "persisted_local");
    assert_eq!(registro.at_unix, 1_700_000_123, "o ACK guarda o instante real");
    assert!(registro.at_unix > 0, "sos_ack.at_unix nunca é 0");
}

#[test]
fn persist_refuses_zero_instant() {
    let mut inc = Incident::triggered("inc-1", "tenant-1", "principal-42");
    assert_eq!(
        inc.persist("inbox:tenant-1:3-7-42", "policy:sos:br:12m", 0),
        Err(IncidentError::MissingTimestamp)
    );
    assert_eq!(inc.state(), IncidentState::TriggeredLocally);
    assert!(inc.acks().is_empty());
}

fn closed_incident() -> Incident {
    let mut inc = securer_incident();
    inc.mark_technical_ack("node-a", 1_700_000_010).unwrap();
    inc.mark_acknowledged_human(HumanProof::person("resp-9", 1_700_000_020)).unwrap();
    inc.close("encerrado pelo operador").unwrap();
    inc
}
