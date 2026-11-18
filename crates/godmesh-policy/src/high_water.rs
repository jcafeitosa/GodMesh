//! ADR-064 / SPEC-002 §6.3 — alta marca autoritativa contra restore e clone.
//!
//! A prova de frescor vem da **autoridade**; contador no armazenamento comum do
//! dispositivo não é prova de atualidade.

/// Alta marca mantida na autoridade para um vínculo `(principal, dispositivo)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityMark {
    principal: String,
    device: String,
    value: u64,
}

impl AuthorityMark {
    #[must_use]
    pub fn new(principal: &str, device: &str, value: u64) -> Self {
        Self { principal: principal.to_owned(), device: device.to_owned(), value }
    }

    #[must_use]
    pub const fn value(&self) -> u64 {
        self.value
    }

    /// Avalia a evidência trazida pelo dispositivo.
    ///
    /// # Errors
    /// `RestoreSuspected` (valor atrás da marca), `ReplaySuspected` (valor igual),
    /// `CloneSuspected` (outro dispositivo no mesmo vínculo), `NoTrustedSource`.
    pub fn compare(&self, evidence: DeviceEvidence) -> Result<(), HighWaterError> {
        if evidence.principal != self.principal {
            return Err(HighWaterError::PrincipalMismatch);
        }
        if evidence.device != self.device {
            return Err(HighWaterError::CloneSuspected);
        }
        match evidence.value.cmp(&self.value) {
            std::cmp::Ordering::Less => Err(HighWaterError::RestoreSuspected),
            std::cmp::Ordering::Equal => Err(HighWaterError::ReplaySuspected),
            std::cmp::Ordering::Greater => Ok(()),
        }
    }

    /// # Errors
    /// `MarkRegression` — a marca só avança.
    pub fn advance(&mut self, new_value: u64) -> Result<(), HighWaterError> {
        if new_value <= self.value {
            return Err(HighWaterError::MarkRegression);
        }
        self.value = new_value;
        Ok(())
    }
}

/// Evidência de frescor apresentada pelo dispositivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceEvidence {
    principal: String,
    device: String,
    value: u64,
}

impl DeviceEvidence {
    #[must_use]
    pub fn new(principal: &str, device: &str, value: u64) -> Self {
        Self { principal: principal.to_owned(), device: device.to_owned(), value }
    }

    /// Dispositivo sem contador protegido não pode provar frescor.
    ///
    /// # Errors
    /// Sempre `NoTrustedSource`.
    pub fn without_protected_counter(_principal: &str, _device: &str) -> Result<Self, HighWaterError> {
        Err(HighWaterError::NoTrustedSource)
    }
}

/// Erros de prova anti-rollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighWaterError {
    RestoreSuspected,
    ReplaySuspected,
    CloneSuspected,
    PrincipalMismatch,
    MarkRegression,
    NoTrustedSource,
}
