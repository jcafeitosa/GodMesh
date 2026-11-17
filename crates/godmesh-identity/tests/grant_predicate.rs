//! ADR-059/ADR-064 — predicado do grant offline; SPEC-002 §6.1–6.3.
//! Regra sob teste: sem F1/F2/F3 demonstrado, sem epoch comparável, com revogação
//! desconhecida ou com classe de retenção ausente, a operação é NEGADA.

use godmesh_identity::grant::{FreshnessSource, GrantDecision, GrantPredicate, OfflineOp, RevocationState};

fn base() -> GrantPredicate {
    GrantPredicate {
        operation: OfflineOp::OpenSosIncident,
        epoch_comparable: true,
        revocation: RevocationState::Absent,
        freshness: FreshnessSource::F3VerifiedOnline,
        retention_class_known: true,
        action_enumerated: true,
    }
}

#[test]
fn allows_when_every_clause_holds() {
    assert_eq!(base().evaluate(), GrantDecision::Allow);
}

#[test]
fn denies_without_any_trusted_freshness_source() {
    let p = GrantPredicate { freshness: FreshnessSource::None, ..base() };
    assert_eq!(p.evaluate(), GrantDecision::DenyDeniedNoTrustedTime);
}

#[test]
fn denies_when_revocation_is_unknown() {
    let p = GrantPredicate { revocation: RevocationState::Unknown, ..base() };
    assert_eq!(p.evaluate(), GrantDecision::DenyRevocationUnknown);
}

#[test]
fn denies_when_epoch_is_incomparable() {
    let p = GrantPredicate { epoch_comparable: false, ..base() };
    assert_eq!(p.evaluate(), GrantDecision::DenyEpochIncomparable);
}

#[test]
fn denies_operation_not_enumerated() {
    let p = GrantPredicate { action_enumerated: false, ..base() };
    assert_eq!(p.evaluate(), GrantDecision::DenyNotEnumerated);
}

#[test]
fn denies_precise_location_sharing_offline_even_when_other_clauses_hold() {
    let p = GrantPredicate { operation: OfflineOp::SharePreciseLocation, ..base() };
    assert_eq!(p.evaluate(), GrantDecision::DenyNeverOffline);
}

#[test]
fn denies_when_retention_class_is_unknown() {
    let p = GrantPredicate { retention_class_known: false, ..base() };
    assert_eq!(p.evaluate(), GrantDecision::DenyRetentionPolicyMissing);
}

#[test]
fn sos_offline_is_denied_without_f1_f2_or_online_check() {
    let mut p = base();
    p.freshness = FreshnessSource::None;
    assert!(matches!(p.evaluate(), GrantDecision::DenyDeniedNoTrustedTime));
    // e a negação não vira "enfileirado": não existe variante de sucesso com fila
    assert!(!matches!(p.evaluate(), GrantDecision::Allow));
}
