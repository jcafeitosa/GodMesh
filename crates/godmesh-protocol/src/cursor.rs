//! ADR-057 — cursor opaco versionado vinculado a escopo, principal e filtros.
//!
//! Cursor sem partição ou de epoch incomparável é **rejeitado**; não existe caminho
//! de compatibilidade que preencha a partição ausente nem que traduza epoch.

/// Sentinela de "sem partição" no contrato de escopo.
const NO_PARTITION: u32 = u32::MAX;

/// Escopo durável ao qual um cursor pertence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorScope {
    tenant: String,
    stream: String,
    partition: u32,
    authority_epoch: u64,
}

impl CursorScope {
    /// # Errors
    /// `CursorError::PartitionRequired` quando a partição é a sentinela de ausência.
    pub fn new(tenant: &str, stream: &str, partition: u32, authority_epoch: u64) -> Result<Self, CursorError> {
        if partition == NO_PARTITION {
            return Err(CursorError::PartitionRequired);
        }
        Ok(Self { tenant: tenant.to_owned(), stream: stream.to_owned(), partition, authority_epoch })
    }

    #[must_use]
    pub const fn partition(&self) -> u32 {
        self.partition
    }

    #[must_use]
    pub const fn authority_epoch(&self) -> u64 {
        self.authority_epoch
    }
}

/// Cursor de leitura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    scope: CursorScope,
    principal: String,
    filters: String,
    profile: String,
    retention: String,
    version: u16,
}

impl Cursor {
    /// # Errors
    /// Propaga erro de escopo e recusa campos obrigatórios vazios.
    pub fn new(
        scope: CursorScope,
        principal: &str,
        filters: &str,
        profile: &str,
        retention: &str,
    ) -> Result<Self, CursorError> {
        if principal.is_empty() || profile.is_empty() || retention.is_empty() {
            return Err(CursorError::EmptyField);
        }
        Ok(Self {
            scope,
            principal: principal.to_owned(),
            filters: filters.to_owned(),
            profile: profile.to_owned(),
            retention: retention.to_owned(),
            version: 1,
        })
    }

    #[must_use]
    pub const fn partition(&self) -> u32 {
        self.scope.partition()
    }

    #[must_use]
    pub const fn authority_epoch(&self) -> u64 {
        self.scope.authority_epoch()
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub fn is_comparable_with(&self, other: &CursorScope) -> bool {
        self.scope.authority_epoch == other.authority_epoch
            && self.scope.partition == other.partition
            && self.scope.tenant == other.tenant
            && self.scope.stream == other.stream
    }

    /// # Errors
    /// `EpochIncomparable`, `PartitionMismatch` ou `TenantMismatch` conforme a divergência.
    pub fn check_against(&self, current: &CursorScope) -> Result<(), CursorError> {
        if self.scope.tenant != current.tenant || self.scope.stream != current.stream {
            return Err(CursorError::TenantMismatch);
        }
        if self.scope.partition != current.partition {
            return Err(CursorError::PartitionMismatch);
        }
        if self.scope.authority_epoch != current.authority_epoch {
            return Err(CursorError::EpochIncomparable);
        }
        Ok(())
    }

    /// # Errors
    /// `PrincipalMismatch` ou `FilterMismatch` quando o cursor é reapresentado fora do vínculo.
    pub fn check_binding(&self, principal: &str, filters: &str) -> Result<(), CursorError> {
        if self.principal != principal {
            return Err(CursorError::PrincipalMismatch);
        }
        if self.filters != filters {
            return Err(CursorError::FilterMismatch);
        }
        Ok(())
    }
}

/// Erros de contrato de cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorError {
    PartitionRequired,
    EpochIncomparable,
    PartitionMismatch,
    TenantMismatch,
    PrincipalMismatch,
    FilterMismatch,
    EmptyField,
}
