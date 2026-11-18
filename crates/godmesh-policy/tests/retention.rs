//! ADR-062/065 — retenção fail-closed por classe.
//! Regra sob teste: classe sem política não persiste; tenant reduz, nunca amplia.

use godmesh_policy::retention::{Jurisdiction, RetentionError, RetentionPolicy, RetentionTable};

fn tabela() -> RetentionTable {
    let mut t = RetentionTable::new(Jurisdiction::Br);
    t.set(RetentionPolicy::new("mensagem", "historico", 24, "expiry", Jurisdiction::Br).unwrap()).unwrap();
    t.set(RetentionPolicy::new("posicao", "mapa", 1, "expiry", Jurisdiction::Br).unwrap()).unwrap();
    t
}

#[test]
fn known_class_returns_policy() {
    let t = tabela();
    let p = t.get("mensagem").expect("classe declarada");
    assert_eq!(p.months(), 24);
    assert_eq!(p.trigger(), "expiry");
}

#[test]
fn unknown_class_fails_closed() {
    assert_eq!(tabela().get("audio_ptt").err(), Some(RetentionError::PolicyMissing));
}

#[test]
fn tenant_may_reduce_days_but_not_extend() {
    let t = tabela();
    assert!(t.tenant_override("mensagem", 6).is_ok());
    assert_eq!(t.tenant_override("mensagem", 36).err(), Some(RetentionError::TenantCannotExtend));
}

#[test]
fn cross_jurisdiction_copy_without_policy_is_refused() {
    let mut t = tabela();
    assert_eq!(
        t.grant_copy("posicao", Jurisdiction::Co).err(),
        Some(RetentionError::CopyJurisdictionNotDeclared)
    );
    t.set(RetentionPolicy::new("posicao", "mapa", 1, "expiry", Jurisdiction::Co).unwrap()).unwrap();
    assert!(t.grant_copy("posicao", Jurisdiction::Co).is_ok());
}

#[test]
fn zero_months_is_refused() {
    assert_eq!(
        RetentionPolicy::new("x", "y", 0, "expiry", Jurisdiction::Br).err(),
        Some(RetentionError::EmptyPeriod)
    );
}

#[test]
fn policy_must_declare_finality_trigger_and_jurisdiction() {
    assert_eq!(
        RetentionPolicy::new("", "y", 1, "expiry", Jurisdiction::Br).err(),
        Some(RetentionError::EmptyField)
    );
    assert_eq!(
        RetentionPolicy::new("x", "", 1, "", Jurisdiction::Br).err(),
        Some(RetentionError::EmptyField)
    );
}
