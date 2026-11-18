//! ADR-062/ADR-065 — retenção fail-closed por classe, com as seis dimensões
//! (finalidade, classe, cópia, jurisdição, prazo, gatilho).

use std::collections::HashMap;

/// Jurisdição declarada da cópia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Jurisdiction {
    Br,
    Co,
}

/// Política de retenção de uma classe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionPolicy {
    class: String,
    finality: String,
    months: u16,
    trigger: String,
    jurisdiction: Jurisdiction,
}

impl RetentionPolicy {
    /// # Errors
    /// `EmptyField` para campos obrigatórios vazios; `EmptyPeriod` para prazo zero.
    pub fn new(
        class: &str,
        finality: &str,
        months: u16,
        trigger: &str,
        jurisdiction: Jurisdiction,
    ) -> Result<Self, RetentionError> {
        if class.is_empty() || finality.is_empty() || trigger.is_empty() {
            return Err(RetentionError::EmptyField);
        }
        if months == 0 {
            return Err(RetentionError::EmptyPeriod);
        }
        Ok(Self {
            class: class.to_owned(),
            finality: finality.to_owned(),
            months,
            trigger: trigger.to_owned(),
            jurisdiction,
        })
    }

    #[must_use]
    pub fn class(&self) -> &str {
        &self.class
    }

    #[must_use]
    pub const fn months(&self) -> u16 {
        self.months
    }

    #[must_use]
    pub fn trigger(&self) -> &str {
        &self.trigger
    }
}

/// Tabela de políticas por classe e jurisdição.
#[derive(Debug, Clone)]
pub struct RetentionTable {
    default_jurisdiction: Jurisdiction,
    entries: HashMap<(String, Jurisdiction), RetentionPolicy>,
}

impl RetentionTable {
    #[must_use]
    pub fn new(default_jurisdiction: Jurisdiction) -> Self {
        Self { default_jurisdiction, entries: HashMap::new() }
    }

    /// # Errors
    /// Propaga erro de política inválida.
    pub fn set(&mut self, policy: RetentionPolicy) -> Result<(), RetentionError> {
        let key = (policy.class.clone(), policy.jurisdiction);
        self.entries.insert(key, policy);
        Ok(())
    }

    /// # Errors
    /// `PolicyMissing` — classe sem política não persiste.
    pub fn get(&self, class: &str) -> Result<&RetentionPolicy, RetentionError> {
        self.entries
            .get(&(class.to_owned(), self.default_jurisdiction))
            .ok_or(RetentionError::PolicyMissing)
    }

    /// Redução de prazo por tenant. Reduzir é permitido; ampliar não.
    ///
    /// # Errors
    /// `PolicyMissing` se a classe não existe; `TenantCannotExtend` se `months`
    /// excede o prazo da política.
    pub fn tenant_override(&self, class: &str, months: u16) -> Result<u16, RetentionError> {
        let policy = self.get(class)?;
        if months > policy.months {
            return Err(RetentionError::TenantCannotExtend);
        }
        Ok(months)
    }

    /// Autoriza (ou não) manter cópia em outra jurisdição.
    ///
    /// # Errors
    /// `PolicyMissing` sem política na origem; `CopyJurisdictionNotDeclared` sem
    /// política explícita para a jurisdição da cópia.
    pub fn grant_copy(&self, class: &str, jurisdiction: Jurisdiction) -> Result<(), RetentionError> {
        let origin = self.get(class)?;
        if origin.jurisdiction == jurisdiction {
            return Ok(());
        }
        self.entries
            .get(&(class.to_owned(), jurisdiction))
            .map(|_| ())
            .ok_or(RetentionError::CopyJurisdictionNotDeclared)
    }
}

/// Erros de política de retenção.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionError {
    EmptyField,
    EmptyPeriod,
    PolicyMissing,
    TenantCannotExtend,
    CopyJurisdictionNotDeclared,
}
