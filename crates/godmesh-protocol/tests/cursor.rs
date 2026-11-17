//! ADR-057 — posição canônica e cursor versionado; SPEC-004 §5, contrato eventos-sincronizacao v0.2.
//! Regra sob teste: cursor sem partição ou de epoch incomparável é rejeitado, nunca traduzido.

use godmesh_protocol::cursor::{Cursor, CursorError, CursorScope};

fn escopo() -> CursorScope {
    CursorScope::new("tenant-1", "stream-9", 3, 7).expect("escopo válido")
}

#[test]
fn builds_cursor_with_partition_and_epoch() {
    let c = Cursor::new(escopo(), "principal-42", "filtro=a", "profile-v1", "retencao-v1").expect("cursor válido");
    assert_eq!(c.partition(), 3);
    assert_eq!(c.authority_epoch(), 7);
    assert!(c.is_comparable_with(&escopo()));
}

#[test]
fn refuses_cursor_without_partition() {
    let erro = CursorScope::new("tenant-1", "stream-9", u32::MAX, 7).err();
    assert_eq!(erro, Some(CursorError::PartitionRequired));
}

#[test]
fn refuses_cursor_with_incomparable_epoch() {
    let c = Cursor::new(escopo(), "principal-42", "filtro=a", "profile-v1", "retencao-v1").unwrap();
    let outro = CursorScope::new("tenant-1", "stream-9", 3, 8).unwrap();
    assert_eq!(c.check_against(&outro), Err(CursorError::EpochIncomparable));
}

#[test]
fn refuses_cursor_from_other_partition() {
    let c = Cursor::new(escopo(), "principal-42", "filtro=a", "profile-v1", "retencao-v1").unwrap();
    let outra = CursorScope::new("tenant-1", "stream-9", 4, 7).unwrap();
    assert_eq!(c.check_against(&outra), Err(CursorError::PartitionMismatch));
}

#[test]
fn cursor_is_bound_to_principal_and_filters() {
    let c = Cursor::new(escopo(), "principal-42", "filtro=a", "profile-v1", "retencao-v1").unwrap();
    assert_eq!(c.check_binding("principal-42", "filtro=a"), Ok(()));
    assert_eq!(c.check_binding("principal-99", "filtro=a"), Err(CursorError::PrincipalMismatch));
    assert_eq!(c.check_binding("principal-42", "filtro=b"), Err(CursorError::FilterMismatch));
}
