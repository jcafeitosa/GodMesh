//! ADR-059/ADR-064 — predicado do grant offline e negações explícitas.
//!
//! Cinco operações são permitidas offline, e apenas com fonte de frescor confiável,
//! epoch comparável, ausência de revogação e classe de retenção conhecida.

/// Operação solicitada sob grant offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflineOp {
    OpenSosIncident,
    GuardianCheckIn,
    SendMessageToAuthorizedChannel,
    RecordLocalObservation,
    LocalPtt,
    SharePreciseLocation,
    FederatedMutation,
    CreateOrEditGrant,
}

impl OfflineOp {
    /// Operações sempre negadas offline, independentemente das demais cláusulas.
    #[must_use]
    pub const fn never_offline(self) -> bool {
        matches!(
            self,
            Self::SharePreciseLocation | Self::FederatedMutation | Self::CreateOrEditGrant
        )
    }
}

/// Fonte de frescor disponível no dispositivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreshnessSource {
    /// Sem fonte confiável: nenhuma operação condicional é autorizada.
    None,
    /// F1 — contador monotônico protegido por hardware.
    F1HardwareCounter,
    /// F2 — âncora de tempo confiável que funciona sem rede.
    F2TrustedTime,
    /// F3 — verificação online concluída.
    F3VerifiedOnline,
}

/// Estado de revogação conhecido no momento da decisão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationState {
    /// Evidência positiva de que não há revogação para o escopo e instante.
    Absent,
    /// Revogação conhecida: nega.
    Known,
    /// Não se sabe: nega (desconhecido não é ausência).
    Unknown,
}

/// Entradas do predicado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantPredicate {
    pub operation: OfflineOp,
    pub epoch_comparable: bool,
    pub revocation: RevocationState,
    pub freshness: FreshnessSource,
    pub retention_class_known: bool,
    pub action_enumerated: bool,
}

/// Resultado do predicado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantDecision {
    Allow,
    DenyNeverOffline,
    DenyNotEnumerated,
    DenyDeniedNoTrustedTime,
    DenyEpochIncomparable,
    DenyRevocationUnknown,
    DenyRetentionPolicyMissing,
}

impl GrantPredicate {
    /// Avalia o predicado. Negar é o padrão; cada cláusula é obrigatória.
    #[must_use]
    pub const fn evaluate(&self) -> GrantDecision {
        if self.operation.never_offline() {
            return GrantDecision::DenyNeverOffline;
        }
        if !self.action_enumerated {
            return GrantDecision::DenyNotEnumerated;
        }
        if matches!(self.freshness, FreshnessSource::None) {
            return GrantDecision::DenyDeniedNoTrustedTime;
        }
        if !self.epoch_comparable {
            return GrantDecision::DenyEpochIncomparable;
        }
        if matches!(self.revocation, RevocationState::Unknown | RevocationState::Known) {
            return GrantDecision::DenyRevocationUnknown;
        }
        if !self.retention_class_known {
            return GrantDecision::DenyRetentionPolicyMissing;
        }
        GrantDecision::Allow
    }
}
