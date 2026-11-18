//! ADR-056 — compatibilidade N/N-1 por perfil, rejeição antes do efeito.
//! Regra sob teste: versão fora da janela declarada é rejeitada antes de qualquer
//! efeito; feature crítica ausente nunca vira downgrade silencioso.

use godmesh_protocol::version::{CompatibilityProfile, Negotiation, VersionError, WireVersion};

fn perfil() -> CompatibilityProfile {
    CompatibilityProfile::new(WireVersion::new(3, 0), 2).expect("perfil válido")
}

#[test]
fn profile_covers_current_and_previous_major() {
    let p = perfil();
    assert!(p.supports(WireVersion::new(3, 0)));
    assert!(p.supports(WireVersion::new(2, 7)));
    assert!(!p.supports(WireVersion::new(1, 9)), "duas versões atrás está fora da janela");
    assert!(!p.supports(WireVersion::new(4, 0)), "versão futura desconhecida");
}

#[test]
fn zero_window_is_refused() {
    assert_eq!(CompatibilityProfile::new(WireVersion::new(1, 0), 0).err(), Some(VersionError::EmptyWindow));
}

#[test]
fn negotiation_accepts_peer_inside_window() {
    let n = Negotiation::run(&perfil(), WireVersion::new(2, 4), &["ack_class".to_owned(), "cursor_v2".to_owned()]);
    assert_eq!(n, Ok(WireVersion::new(2, 4)));
}

#[test]
fn negotiation_refuses_peer_outside_window_before_effect() {
    let n = Negotiation::run(&perfil(), WireVersion::new(1, 0), &["ack_class".to_owned()]);
    assert_eq!(n, Err(VersionError::PeerOutsideWindow));
}

#[test]
fn missing_critical_feature_is_refused_not_silently_downgraded() {
    let mut p = perfil();
    p.require_critical("ack_class");
    let n = Negotiation::run(&p, WireVersion::new(3, 0), &["cursor_v2".to_owned()]);
    assert_eq!(n, Err(VersionError::MissingCriticalFeature));
}

#[test]
fn optional_feature_absence_is_not_an_error() {
    let mut p = perfil();
    p.require_critical("ack_class");
    p.require_optional("srtp_profile");
    let n = Negotiation::run(&p, WireVersion::new(3, 0), &["ack_class".to_owned()]);
    assert_eq!(n, Ok(WireVersion::new(3, 0)));
}
