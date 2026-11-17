//! ADR-058 — classes de ACK por fronteira.
//!
//! A classe é sempre **evidência do que aconteceu numa fronteira**, nunca promoção
//! inferida. Ordem: transporte < persistência local < replicação à autoridade <
//! materialização < entrega a dispositivo < reconhecimento humano.

/// Fronteira comprovada por uma confirmação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AckClass {
    TransportAccepted,
    PersistedLocal,
    ReplicatedToAuthority,
    Materialized,
    DeliveredToDevice,
    AcknowledgedHuman,
}

impl AckClass {
    /// Converte a representação textual de contrato.
    ///
    /// # Errors
    /// `MissingClass` para string vazia; `UnknownClass` para qualquer nome fora do
    /// conjunto fechado da ADR-058.
    pub fn parse(text: &str) -> Result<Self, AckError> {
        match text {
            "transport_accepted" => Ok(Self::TransportAccepted),
            "persisted_local" => Ok(Self::PersistedLocal),
            "replicated_to_authority" => Ok(Self::ReplicatedToAuthority),
            "materialized" => Ok(Self::Materialized),
            "delivered_to_device" => Ok(Self::DeliveredToDevice),
            "acknowledged_human" => Ok(Self::AcknowledgedHuman),
            "" => Err(AckError::MissingClass),
            _ => Err(AckError::UnknownClass),
        }
    }

    /// Nome canônico usado em resposta de API.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TransportAccepted => "transport_accepted",
            Self::PersistedLocal => "persisted_local",
            Self::ReplicatedToAuthority => "replicated_to_authority",
            Self::Materialized => "materialized",
            Self::DeliveredToDevice => "delivered_to_device",
            Self::AcknowledgedHuman => "acknowledged_human",
        }
    }
}

/// Evidência coletada numa fronteira concreta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckEvidence {
    class: AckClass,
    source: String,
}

impl AckEvidence {
    #[must_use]
    pub fn new(class: AckClass, source: &str) -> Self {
        Self { class, source: source.to_owned() }
    }

    #[must_use]
    pub const fn class(&self) -> AckClass {
        self.class
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Confirma que é legítimo apresentar `target` ao consumidor.
    ///
    /// Só é permitido rebaixar (alegar classe igual ou inferior). Alegar classe
    /// superior é erro de contrato, não aviso.
    ///
    /// # Errors
    /// `AckError::PromotionRefused` quando `target` é superior à evidência.
    pub fn claim(&self, target: AckClass) -> Result<AckClass, AckError> {
        if target > self.class {
            return Err(AckError::PromotionRefused { from: self.class, to: target });
        }
        Ok(target)
    }
}

/// Erros de contrato de ACK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckError {
    MissingClass,
    UnknownClass,
    PromotionRefused { from: AckClass, to: AckClass },
}
