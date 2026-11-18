//! ADR-061 — outbox transacional com prefixo contíguo e replay sem efeito externo.
//!
//! Publicar não é confirmar. Confirmar só anda em ordem. Replay reconstrói projeção
//! e **nunca** reemite efeito irreversível (bridge, rádio, pagamento, notificação).

/// Natureza do efeito registrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    /// Evento de domínio: pode ser reconstruído por replay.
    Event,
    /// Efeito irreversível: replay não reemite.
    Irreversible(&'static str),
}

/// Desfecho de uma tentativa de publicação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishOutcome {
    Published(u64),
    NothingToPublish,
}

/// Plano de replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplayPlan {
    pub entries: usize,
    pub first_sequence: Option<u64>,
    pub skipped_irreversible: usize,
}

/// Outbox de uma partição.
#[derive(Debug, Clone)]
pub struct Outbox {
    origin: String,
    tenant: String,
    stream: String,
    partition: u32,
    epoch: u64,
    entries: Vec<(u64, EffectKind, bool)>,
    confirmed: u64,
}

impl Outbox {
    #[must_use]
    pub fn new(origin: &str, tenant: &str, stream: &str, partition: u32, epoch: u64) -> Self {
        Self {
            origin: origin.to_owned(),
            tenant: tenant.to_owned(),
            stream: stream.to_owned(),
            partition,
            epoch,
            entries: Vec::new(),
            confirmed: 0,
        }
    }

    /// # Errors
    /// `DuplicateSequence` para sequência repetida; `NonContiguousAppend` para salto.
    pub fn append(&mut self, seq: u64, kind: EffectKind) -> Result<(), OutboxError> {
        if self.entries.iter().any(|(s, _, _)| *s == seq) {
            return Err(OutboxError::DuplicateSequence);
        }
        let proxima = self.entries.last().map_or(1, |(s, _, _)| s + 1);
        if seq != proxima {
            return Err(OutboxError::NonContiguousAppend);
        }
        self.entries.push((seq, kind, false));
        Ok(())
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.entries.iter().filter(|(s, _, published)| !*published && *s > self.confirmed).count()
    }

    #[must_use]
    pub const fn last_contiguous(&self) -> u64 {
        self.confirmed
    }

    /// Próxima publicação pendente; marca como publicada, não como confirmada.
    pub fn publish_next(&mut self) -> Option<PublishOutcome> {
        for entry in &mut self.entries {
            if !entry.2 && entry.0 > self.confirmed {
                entry.2 = true;
                return Some(PublishOutcome::Published(entry.0));
            }
        }
        None
    }

    /// # Errors
    /// `ConfirmationOutOfOrder` quando a confirmação salta a sequência esperada.
    pub fn confirm(&mut self, seq: u64) -> Result<(), OutboxError> {
        if seq != self.confirmed + 1 {
            return Err(OutboxError::ConfirmationOutOfOrder);
        }
        self.confirmed = seq;
        Ok(())
    }

    /// Reconstrói projeção a partir de um prefixo confirmado.
    #[must_use]
    pub fn replay_from(&self, confirmed: u64) -> ReplayPlan {
        let mut plan = ReplayPlan::default();
        for (seq, kind, _) in &self.entries {
            if *seq <= confirmed {
                continue;
            }
            match kind {
                EffectKind::Event => {
                    plan.entries += 1;
                    plan.first_sequence.get_or_insert(*seq);
                }
                EffectKind::Irreversible(_) => plan.skipped_irreversible += 1,
            }
        }
        plan
    }

    #[must_use]
    pub fn partition(&self) -> u32 {
        self.partition
    }

    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    #[must_use]
    pub fn labels(&self) -> (&str, &str, &str) {
        (&self.origin, &self.tenant, &self.stream)
    }
}

/// Erros do outbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxError {
    DuplicateSequence,
    NonContiguousAppend,
    ConfirmationOutOfOrder,
}
