//! ADR-061 — outbox transacional com prefixo contíguo e replay sem efeito externo.
//!
//! Publicar não é confirmar. Confirmar só anda em ordem e só para entrada
//! publicada. Replay **nunca** reemite efeito irreversível (bridge, rádio,
//! pagamento, notificação).
//!
//! Limite declarado: este outbox guarda apenas a **sequência** e a **natureza**
//! do efeito de cada entrada (`EffectKind`). Não guarda payload, identidade de
//! evento nem digest. Por isso `replay_from` devolve um *plano* — quantas
//! entradas de projeção e quantos efeitos irreversíveis foram pulados, e a
//! primeira sequência — e não uma projeção reconstruída. Reconstruir projeção de
//! verdade exige o payload, que depende de SPEC-004/SPEC-021 e não existe aqui.

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
    /// Entrada marcada como publicada.
    Published(u64),
    /// Não havia entrada pendente para publicar.
    NothingToPublish,
}

/// Plano de replay: contagem e primeira sequência, sem conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplayPlan {
    /// Entradas de projeção (`EffectKind::Event`) depois do prefixo.
    pub entries: usize,
    /// Primeira sequência a reproduzir, se houver alguma.
    pub first_sequence: Option<u64>,
    /// Entradas irreversíveis depois do prefixo que o replay **não** reemite.
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

    /// Entradas enfileiradas, não publicadas e acima do prefixo confirmado.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.entries.iter().filter(|(s, _, published)| !*published && *s > self.confirmed).count()
    }

    /// Prefixo contíguo confirmado pelo destino.
    #[must_use]
    pub const fn last_contiguous(&self) -> u64 {
        self.confirmed
    }

    /// Maior sequência enfileirada, se houver alguma.
    #[must_use]
    pub fn last_sequence(&self) -> Option<u64> {
        self.entries.last().map(|(seq, _, _)| *seq)
    }

    /// Próxima publicação pendente; marca como publicada, não como confirmada.
    ///
    /// Devolve `NothingToPublish` quando não há entrada anunciada e ainda não
    /// publicada acima do prefixo confirmado.
    pub fn publish_next(&mut self) -> PublishOutcome {
        for entry in &mut self.entries {
            if !entry.2 && entry.0 > self.confirmed {
                entry.2 = true;
                return PublishOutcome::Published(entry.0);
            }
        }
        PublishOutcome::NothingToPublish
    }

    /// Confirma uma sequência: ela precisa existir, estar publicada e ser a
    /// próxima do prefixo (`confirmed + 1`).
    ///
    /// # Errors
    /// `ConfirmationWithoutEntry` sem entrada correspondente;
    /// `ConfirmationOfUnpublished` para entrada ainda não publicada;
    /// `ConfirmationOutOfOrder` quando a confirmação salta a sequência esperada.
    /// Em todos os casos `confirmed` não muda.
    pub fn confirm(&mut self, seq: u64) -> Result<(), OutboxError> {
        let Some((_, _, published)) = self.entries.iter().find(|(s, _, _)| *s == seq) else {
            return Err(OutboxError::ConfirmationWithoutEntry);
        };
        if !*published {
            return Err(OutboxError::ConfirmationOfUnpublished);
        }
        if seq != self.confirmed + 1 {
            return Err(OutboxError::ConfirmationOutOfOrder);
        }
        self.confirmed = seq;
        Ok(())
    }

    /// Plano de replay a partir de um prefixo **já confirmado**.
    ///
    /// Prefixo anterior ao confirmado é permitido e reproduz mais (recuperação
    /// conservadora, sempre sobre sequências). Prefixo à frente do confirmado é
    /// recuperação incompleta: o chamador está assumindo um reconhecimento que o
    /// destino nunca deu, e devolver um plano vazio seria indistinguível de
    /// "nada a reproduzir".
    ///
    /// # Errors
    /// `RecoveryIncomplete` quando `prefix > confirmed`.
    pub fn replay_from(&self, prefix: u64) -> Result<ReplayPlan, OutboxError> {
        if prefix > self.confirmed {
            return Err(OutboxError::RecoveryIncomplete {
                requested: prefix,
                confirmed: self.confirmed,
            });
        }
        let mut plan = ReplayPlan::default();
        for (seq, kind, _) in &self.entries {
            if *seq <= prefix {
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
        Ok(plan)
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
    /// A confirmação não é a próxima do prefixo.
    ConfirmationOutOfOrder,
    /// Não existe entrada com a sequência confirmada.
    ConfirmationWithoutEntry,
    /// A entrada existe mas nunca foi publicada.
    ConfirmationOfUnpublished,
    /// O prefixo pedido para replay está à frente do confirmado.
    RecoveryIncomplete { requested: u64, confirmed: u64 },
}