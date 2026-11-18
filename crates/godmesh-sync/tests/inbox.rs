//! ADR-057/061 — Inbox durável: dedupe por identidade, lacuna exige rebootstrap,
//! ACK máximo é `persisted_local`.

use godmesh_sync::inbox::{Inbox, InboxError, InboxOutcome};
use godmesh_sync::position::EventPosition;

fn p(seq: u64) -> EventPosition {
    EventPosition::new("node-a", "tenant-1", "stream-9", 1, 7, seq).unwrap()
}

#[test]
fn accepts_contiguous_events_and_reports_ack_class() {
    let mut ib = Inbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    assert_eq!(ib.accept(p(1)), Ok(InboxOutcome::Accepted { ack_class: "persisted_local", contiguous_prefix: 1 }));
    assert_eq!(ib.accept(p(2)), Ok(InboxOutcome::Accepted { ack_class: "persisted_local", contiguous_prefix: 2 }));
    assert_eq!(ib.contiguous_prefix(), 2);
}

#[test]
fn duplicate_event_is_deduplicated_not_reprocessed() {
    let mut ib = Inbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    ib.accept(p(1)).unwrap();
    assert_eq!(ib.accept(p(1)), Ok(InboxOutcome::Duplicate));
    assert_eq!(ib.contiguous_prefix(), 1);
}

#[test]
fn gap_is_reported_and_does_not_advance_prefix() {
    let mut ib = Inbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    ib.accept(p(1)).unwrap();
    assert_eq!(ib.accept(p(3)), Ok(InboxOutcome::Gap { expected: 2 }));
    assert_eq!(ib.contiguous_prefix(), 1, "lacuna nunca é saltada");
}

#[test]
fn event_from_other_partition_is_refused() {
    let mut ib = Inbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    let outra = EventPosition::new("node-a", "tenant-1", "stream-9", 2, 7, 1).unwrap();
    assert_eq!(ib.accept(outra), Err(InboxError::PartitionMismatch));
}

#[test]
fn event_from_other_epoch_is_refused() {
    let mut ib = Inbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    let antigo = EventPosition::new("node-a", "tenant-1", "stream-9", 1, 6, 1).unwrap();
    assert_eq!(ib.accept(antigo), Err(InboxError::EpochIncomparable));
}

// --- retomada a partir do prefixo durável (POST /v1/sync/inbox) --------------

#[test]
fn resuming_from_a_durable_prefix_treats_covered_positions_as_duplicates() {
    let mut ib = Inbox::resuming("node-a", "tenant-1", "stream-9", 1, 7, 42);
    assert_eq!(ib.accept(p(42)), Ok(InboxOutcome::Duplicate), "já coberta pelo prefixo");
    assert_eq!(ib.accept(p(7)), Ok(InboxOutcome::Duplicate), "muito atrás do prefixo");
    assert_eq!(ib.contiguous_prefix(), 42, "repetição nunca regride o prefixo");
}

#[test]
fn resuming_only_accepts_the_position_right_after_the_durable_prefix() {
    let mut ib = Inbox::resuming("node-a", "tenant-1", "stream-9", 1, 7, 42);
    assert_eq!(ib.accept(p(44)), Ok(InboxOutcome::Gap { expected: 43 }));
    assert_eq!(
        ib.accept(p(43)),
        Ok(InboxOutcome::Accepted { ack_class: "persisted_local", contiguous_prefix: 43 })
    );
    assert_eq!(ib.contiguous_prefix(), 43);
}

#[test]
fn resuming_from_zero_behaves_like_a_fresh_inbox() {
    let mut retomada = Inbox::resuming("node-a", "tenant-1", "stream-9", 1, 7, 0);
    let mut nova = Inbox::new("node-a", "tenant-1", "stream-9", 1, 7);
    assert_eq!(retomada.accept(p(1)), nova.accept(p(1)));
    assert_eq!(retomada.contiguous_prefix(), nova.contiguous_prefix());
}
