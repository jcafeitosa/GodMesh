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

// --- token opaco de cursor ---------------------------------------------------

use godmesh_protocol::cursor::CursorTokenError;

fn cursor_com_campos(tenant: &str, stream: &str, principal: &str, filtro: &str) -> Cursor {
    let escopo = CursorScope::new(tenant, stream, 3, 7).expect("escopo válido");
    Cursor::new(escopo, principal, filtro, "gmp.event.v1", "BR").expect("cursor válido")
}

#[test]
fn token_survives_the_round_trip_with_position() {
    let cursor = cursor_com_campos("tenant-1", "stream-9", "principal-42", "origin_node=*");
    let token = cursor.encode(43);
    assert!(
        token.starts_with("v1."),
        "token precisa declarar a versão: {token}"
    );

    let decodificado = Cursor::decode(&token).expect("token emitido precisa ser legível");
    assert_eq!(decodificado.last_seq(), 43);
    assert_eq!(decodificado.cursor(), &cursor);
    assert!(decodificado.cursor().is_comparable_with(&escopo()));
}

#[test]
fn token_is_bound_to_partition_epoch_and_filters() {
    let token =
        cursor_com_campos("tenant-1", "stream-9", "principal-42", "origin_node=*").encode(43);
    let decodificado = Cursor::decode(&token).unwrap();
    let c = decodificado.cursor();

    assert_eq!(
        c.check_against(&CursorScope::new("tenant-1", "stream-9", 4, 7).unwrap()),
        Err(CursorError::PartitionMismatch),
        "cursor de outra partição nunca é traduzido"
    );
    assert_eq!(
        c.check_against(&CursorScope::new("tenant-1", "stream-9", 3, 8).unwrap()),
        Err(CursorError::EpochIncomparable)
    );
    assert_eq!(
        c.check_binding("principal-42", "origin_node=node-a"),
        Err(CursorError::FilterMismatch)
    );
    assert_eq!(
        c.check_binding("principal-99", "origin_node=*"),
        Err(CursorError::PrincipalMismatch)
    );
}

#[test]
fn fields_with_separators_are_not_ambiguous() {
    let cursor = cursor_com_campos(
        "tenant|1:2",
        "stream-9|x",
        "principal-42",
        "origin_node=node:a",
    );
    let token = cursor.encode(1);
    let decodificado = Cursor::decode(&token).expect("tamanho prefixado não é ambíguo");
    assert_eq!(decodificado.cursor(), &cursor);
}

#[test]
fn malformed_or_unknown_version_tokens_are_refused() {
    let token =
        cursor_com_campos("tenant-1", "stream-9", "principal-42", "origin_node=*").encode(43);

    assert_eq!(
        Cursor::decode(""),
        Err(CursorTokenError::Malformed),
        "vazio não é cursor"
    );
    assert_eq!(
        Cursor::decode("nao-e-cursor"),
        Err(CursorTokenError::Malformed)
    );
    assert_eq!(
        Cursor::decode(&token[..token.len() - 3]),
        Err(CursorTokenError::Malformed)
    );

    let outra_versao = token.replacen("v1.", "v2.", 1);
    assert_eq!(
        Cursor::decode(&outra_versao),
        Err(CursorTokenError::UnsupportedVersion(2)),
        "versão desconhecida é recusada, nunca reinterpretada"
    );
}

#[test]
fn token_without_partition_is_refused() {
    // Token montado à mão sem partição: a sentinela nunca vira uma partição válida.
    let escopo = CursorScope::new("tenant-1", "stream-9", 3, 7).unwrap();
    let cursor = Cursor::new(
        escopo,
        "principal-42",
        "origin_node=*",
        "gmp.event.v1",
        "BR",
    )
    .unwrap();
    let token = cursor.encode(1);
    let decoded = Cursor::decode(&token).unwrap();
    assert_eq!(decoded.cursor().partition(), 3);

    let sem_particao = "v1.MToxfDg6dGVuYW50LTF8ODpzdHJlYW0tOXwxMjpwcmluY2lwYWwtNDJ8MTM6b3JpZ2luX25vZGU9KnwxMjpnbXAuZXZlbnQudjF8MjpCUnwxMDo0Mjk0OTY3Mjk1fDE6N3wxOjF8";
    assert_eq!(
        Cursor::decode(sem_particao),
        Err(CursorTokenError::PartitionRequired)
    );
}
