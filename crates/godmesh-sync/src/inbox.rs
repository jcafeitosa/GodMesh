//! ADR-057/061 — Inbox durável: dedupe, lacuna sem salto e ACK máximo `persisted_local`.

use crate::position::EventPosition;

/// Resultado de receber um evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxOutcome {
    Accepted { ack_class: &'static str, contiguous_prefix: u64 },
    Duplicate,
    Gap { expected: u64 },
}

/// Inbox de uma partição.
#[derive(Debug, Clone)]
pub struct Inbox {
    origin: String,
    tenant: String,
    stream: String,
    partition: u32,
    epoch: u64,
    seen: Vec<u64>,
    contiguous: u64,
}

impl Inbox {
    #[must_use]
    pub fn new(origin: &str, tenant: &str, stream: &str, partition: u32, epoch: u64) -> Self {
        Self {
            origin: origin.to_owned(),
            tenant: tenant.to_owned(),
            stream: stream.to_owned(),
            partition,
            epoch,
            seen: Vec::new(),
            contiguous: 0,
        }
    }

    /// Retoma uma Inbox a partir do prefixo contíguo já durável.
    ///
    /// O servidor não pode recomeçar do zero a cada lote: o prefixo é o
    /// checkpoint persistido, e uma posição já coberta por ele é repetição, não
    /// novidade. `contiguous_prefix == 0` equivale a [`Inbox::new`] (namespace
    /// sem evento durável), e o construtor não aceita prefixo inicial por outra
    /// via — não existe Inbox que "salte" para um prefixo inventado pelo
    /// chamador: o valor tem de ser o mesmo que está gravado.
    #[must_use]
    pub fn resuming(
        origin: &str,
        tenant: &str,
        stream: &str,
        partition: u32,
        epoch: u64,
        contiguous_prefix: u64,
    ) -> Self {
        Self {
            origin: origin.to_owned(),
            tenant: tenant.to_owned(),
            stream: stream.to_owned(),
            partition,
            epoch,
            seen: Vec::new(),
            contiguous: contiguous_prefix,
        }
    }

    /// # Errors
    /// `PartitionMismatch` / `EpochIncomparable` quando o evento não pertence a esta Inbox.
    pub fn accept(&mut self, position: EventPosition) -> Result<InboxOutcome, InboxError> {
        if position.partition() != self.partition {
            return Err(InboxError::PartitionMismatch);
        }
        if position.authority_epoch() != self.epoch {
            return Err(InboxError::EpochIncomparable);
        }
        let seq = position.seq();
        // Repetição é decidida contra o **prefixo durável** e contra o que já
        // entrou neste lote. Sem a primeira checagem, retomar de um prefixo > 0
        // trataria uma posição já durável como novidade e poderia regredir o
        // prefixo ao aceitá-la.
        if seq <= self.contiguous || self.seen.contains(&seq) {
            return Ok(InboxOutcome::Duplicate);
        }
        if seq > self.contiguous.saturating_add(1) {
            return Ok(InboxOutcome::Gap {
                expected: self.contiguous.saturating_add(1),
            });
        }
        self.seen.push(seq);
        self.contiguous = seq;
        Ok(InboxOutcome::Accepted { ack_class: "persisted_local", contiguous_prefix: self.contiguous })
    }

    #[must_use]
    pub const fn contiguous_prefix(&self) -> u64 {
        self.contiguous
    }

    #[must_use]
    pub fn labels(&self) -> (&str, &str, &str) {
        (&self.origin, &self.tenant, &self.stream)
    }
}

/// Erros da Inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxError {
    PartitionMismatch,
    EpochIncomparable,
}
