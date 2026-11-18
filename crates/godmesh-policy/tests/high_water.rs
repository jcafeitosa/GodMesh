//! ADR-064/SPEC-002 §6.3 — prova anti-rollback e anti-clone do grant offline.
//! Regra sob teste: a prova vem da autoridade, não do dispositivo; restore e clone
//! são detectados; ausência de fonte de confiança nega.

use godmesh_policy::high_water::{AuthorityMark, DeviceEvidence, HighWaterError};

fn autoridade() -> AuthorityMark {
    AuthorityMark::new("principal-42", "device-7", 1_000)
}

#[test]
fn accepts_evidence_strictly_ahead_of_mark() {
    let a = autoridade();
    assert!(a.compare(DeviceEvidence::new("principal-42", "device-7", 1_050)).is_ok());
}

#[test]
fn detects_restore_with_evidence_behind_mark() {
    let a = autoridade();
    assert_eq!(
        a.compare(DeviceEvidence::new("principal-42", "device-7", 900)),
        Err(HighWaterError::RestoreSuspected)
    );
}

#[test]
fn detects_equal_counter_as_replay() {
    let a = autoridade();
    assert_eq!(
        a.compare(DeviceEvidence::new("principal-42", "device-7", 1_000)),
        Err(HighWaterError::ReplaySuspected)
    );
}

#[test]
fn detects_clone_when_other_device_uses_same_key() {
    let a = autoridade();
    assert_eq!(
        a.compare(DeviceEvidence::new("principal-42", "device-8", 1_100)),
        Err(HighWaterError::CloneSuspected)
    );
}

#[test]
fn mark_advances_only_upwards() {
    let mut a = autoridade();
    assert!(a.advance(1_050).is_ok());
    assert_eq!(a.value(), 1_050);
    assert_eq!(a.advance(1_020), Err(HighWaterError::MarkRegression));
}

#[test]
fn device_without_protected_counter_cannot_prove_anything() {
    assert_eq!(
        DeviceEvidence::without_protected_counter("principal-42", "device-7"),
        Err(HighWaterError::NoTrustedSource)
    );
}
