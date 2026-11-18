//! ADR-061 — outbox transacional, replay sem repetir efeito externo e prefixo contíguo.
//! Regra sob teste: replay reconstrói projeção; nunca reemite efeito irreversível.

use godmesh_sync::outbox::{EffectKind, Outbox, OutboxError, PublishOutcome};

fn pos(seq: u64) -> (&'static str, &'static str, &'static str, u32, u64, u64) {
    ("node-a", "tenant-1", "stream-9", 1, 7, seq)
}

#[test]
fn append_keeps_declaration_order() {
    let mut ob = Outbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    ob.append(pos(1).5, EffectKind::Event).unwrap();
    ob.append(pos(2).5, EffectKind::Event).unwrap();
    assert_eq!(ob.pending(), 2);
    // ADR-061: append enfileira, não confirma. O prefixo contíguo representa
    // recebimento duravelmente reconhecido pelo destino (`confirmed`), então ele
    // só avança em `confirm()` — nunca em `append()`. Ver
    // `publish_advances_prefix_only_when_acknowledged`, que prova o mesmo para
    // `publish_next()`. Asserir 2 aqui seria afirmar que enfileirar é confirmar.
    assert_eq!(ob.last_contiguous(), 0, "append não avança prefixo; só confirm() avança");
    // Ordem de declaração preservada: a publicação sai na ordem em que foi enfileirada.
    assert_eq!(ob.publish_next(), Some(PublishOutcome::Published(1)));
    assert_eq!(ob.publish_next(), Some(PublishOutcome::Published(2)));
    assert_eq!(ob.last_contiguous(), 0, "publicar também não confirma");
}

#[test]
fn duplicate_sequence_is_refused() {
    let mut ob = Outbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    ob.append(1, EffectKind::Event).unwrap();
    assert_eq!(ob.append(1, EffectKind::Event), Err(OutboxError::DuplicateSequence));
}

#[test]
fn gap_in_sequence_is_refused() {
    let mut ob = Outbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    ob.append(1, EffectKind::Event).unwrap();
    assert_eq!(ob.append(3, EffectKind::Event), Err(OutboxError::NonContiguousAppend));
}

#[test]
fn publish_advances_prefix_only_when_acknowledged() {
    let mut ob = Outbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    ob.append(1, EffectKind::Event).unwrap();
    ob.append(2, EffectKind::Event).unwrap();

    assert_eq!(ob.publish_next(), Some(PublishOutcome::Published(1)));
    assert_eq!(ob.last_contiguous(), 0, "publicar não é confirmar");
    ob.confirm(1).unwrap();
    assert_eq!(ob.last_contiguous(), 1);
    assert_eq!(ob.pending(), 1);
}

#[test]
fn replay_never_reemits_irreversible_effect() {
    let mut ob = Outbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    ob.append(1, EffectKind::Irreversible("bridge_tx")).unwrap();
    ob.publish_next();
    // A entrada irreversível precisa continuar NÃO confirmada. `replay_from(p)`
    // considera apenas o que está depois do prefixo confirmado (`seq > p`);
    // confirmar a entrada a excluiria do replay e o teste passaria sem exercitar
    // a regra. O risco real da ADR-061 é reemitir efeito externo de uma entrada
    // que ficou pendente — é exatamente esse caso que provamos aqui.

    let plano = ob.replay_from(0);
    assert_eq!(ob.last_contiguous(), 0, "entrada irreversível segue pendente");
    assert_eq!(plano.entries, 0, "replay reconstrói projeção, não reenvia");
    assert_eq!(plano.skipped_irreversible, 1, "efeito irreversível é contado e pulado");
}

#[test]
fn replay_from_prefix_returns_only_unconfirmed() {
    let mut ob = Outbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    for seq in 1..=3 { ob.append(seq, EffectKind::Event).unwrap(); }
    ob.publish_next(); ob.confirm(1).unwrap();
    let plano = ob.replay_from(1);
    assert_eq!(plano.entries, 2);
    assert_eq!(plano.first_sequence, Some(2));
}

#[test]
fn gap_in_confirmation_is_refused() {
    let mut ob = Outbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    ob.append(1, EffectKind::Event).unwrap();
    ob.append(2, EffectKind::Event).unwrap();
    ob.publish_next();
    assert_eq!(ob.confirm(2), Err(OutboxError::ConfirmationOutOfOrder));
}
