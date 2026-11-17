//! ADR-057 — posição canônica `(origem, tenant, stream, partição, epoch, sequência)`.
//!
//! `event_sequence`, `dispatch_sequence` e `authority_epoch` são tipos distintos;
//! aqui só existe a sequência de evento dentro de uma partição e epoch.

use core::cmp::Ordering;

/// Posição de um evento durável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPosition {
    origin: String,
    tenant: String,
    stream: String,
    partition: u32,
    authority_epoch: u64,
    seq: u64,
}

impl EventPosition {
    /// # Errors
    /// `EmptyField` para identificadores vazios; `ZeroSequence` para sequência 0.
    pub fn new(
        origin: &str,
        tenant: &str,
        stream: &str,
        partition: u32,
        authority_epoch: u64,
        seq: u64,
    ) -> Result<Self, PositionError> {
        if origin.is_empty() || tenant.is_empty() || stream.is_empty() {
            return Err(PositionError::EmptyField);
        }
        if seq == 0 {
            return Err(PositionError::ZeroSequence);
        }
        Ok(Self {
            origin: origin.to_owned(),
            tenant: tenant.to_owned(),
            stream: stream.to_owned(),
            partition,
            authority_epoch,
            seq,
        })
    }

    #[must_use]
    pub const fn partition(&self) -> u32 {
        self.partition
    }

    #[must_use]
    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }

    #[must_use]
    pub const fn seq(&self) -> u64 {
        self.seq
    }

    /// Indica que chegou um evento com lacuna em relação ao esperado.
    ///
    /// O chamador deve pedir rebootstrap; **nunca** saltar a lacuna em silêncio.
    #[must_use]
    pub fn needs_rebootstrap_from(&self, expected: &Self) -> bool {
        self.same_stream(expected) && self.authority_epoch == expected.authority_epoch && self.seq > expected.seq + 1
    }

    fn same_stream(&self, other: &Self) -> bool {
        self.origin == other.origin
            && self.tenant == other.tenant
            && self.stream == other.stream
            && self.partition == other.partition
    }
}

impl Ord for EventPosition {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.origin, &self.tenant, &self.stream, self.partition, self.authority_epoch, self.seq).cmp(&(
            &other.origin,
            &other.tenant,
            &other.stream,
            other.partition,
            other.authority_epoch,
            other.seq,
        ))
    }
}

impl PartialOrd for EventPosition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Erros de contrato de posição.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionError {
    EmptyField,
    ZeroSequence,
}
