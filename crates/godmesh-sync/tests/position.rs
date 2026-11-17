//! ADR-057/061 — posição canônica de evento e unicidade; SPEC-004 §3–5.
//! Regra sob teste: `event_sequence`, `dispatch_sequence` e `authority_epoch` são tipos
//! distintos; duas partições não colidem; replay não promove classe de ACK.

use godmesh_sync::position::{EventPosition, PositionError};

#[test]
fn same_sequence_in_two_partitions_is_not_the_same_position() {
    let a = EventPosition::new("node-a", "tenant-1", "stream-9", 1, 7, 42).unwrap();
    let b = EventPosition::new("node-a", "tenant-1", "stream-9", 2, 7, 42).unwrap();
    assert_ne!(a, b);
    assert!(a < b, "ordenação deve considerar partição");
}

#[test]
fn epoch_takes_precedence_over_sequence_in_ordering() {
    let antigo = EventPosition::new("node-a", "tenant-1", "stream-9", 1, 6, 9_999).unwrap();
    let novo = EventPosition::new("node-a", "tenant-1", "stream-9", 1, 7, 1).unwrap();
    assert!(antigo < novo, "epoch anterior nunca é 'depois' por sequência maior");
}

#[test]
fn refuses_zero_sequence_and_empty_identifiers() {
    assert_eq!(EventPosition::new("node-a", "tenant-1", "stream-9", 1, 7, 0), Err(PositionError::ZeroSequence));
    assert_eq!(EventPosition::new("", "tenant-1", "stream-9", 1, 7, 1), Err(PositionError::EmptyField));
    assert_eq!(EventPosition::new("node-a", "tenant-1", "", 1, 7, 1), Err(PositionError::EmptyField));
}

#[test]
fn cursor_of_gap_requires_rebootstrap_instead_of_silent_skip() {
    let esperado = EventPosition::new("node-a", "tenant-1", "stream-9", 1, 7, 42).unwrap();
    let recebido = EventPosition::new("node-a", "tenant-1", "stream-9", 1, 7, 45).unwrap();
    assert!(recebido.needs_rebootstrap_from(&esperado));
    let seguinte = EventPosition::new("node-a", "tenant-1", "stream-9", 1, 7, 43).unwrap();
    assert!(!seguinte.needs_rebootstrap_from(&esperado));
}
