//! ADR-056 — compatibilidade N/N-1 por perfil versionado.
//!
//! A negociação ocorre **antes de qualquer efeito**. Peer fora da janela ou sem
//! feature crítica é rejeitado; não existe downgrade silencioso.

use core::cmp::Ordering;

/// Versão de wire GMP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireVersion {
    major: u16,
    minor: u16,
}

impl WireVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }
}

impl Ord for WireVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor).cmp(&(other.major, other.minor))
    }
}

impl PartialOrd for WireVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Perfil de compatibilidade publicado: versão corrente e janela N/N-1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityProfile {
    current: WireVersion,
    window: u16,
    critical: Vec<String>,
    optional: Vec<String>,
}

impl CompatibilityProfile {
    /// # Errors
    /// `EmptyWindow` quando a janela é zero.
    pub fn new(current: WireVersion, window: u16) -> Result<Self, VersionError> {
        if window == 0 {
            return Err(VersionError::EmptyWindow);
        }
        Ok(Self { current, window, critical: Vec::new(), optional: Vec::new() })
    }

    /// Verdadeiro quando `v` está dentro da janela declarada.
    ///
    /// Janela `w` cobre a versão corrente e as `w - 1` majors anteriores. Versão
    /// futura nunca é "suportada".
    #[must_use]
    pub fn supports(&self, v: WireVersion) -> bool {
        if v > self.current {
            return false;
        }
        v.major == self.current.major || self.current.major - v.major < self.window
    }

    pub fn require_critical(&mut self, feature: &str) {
        self.critical.push(feature.to_owned());
    }

    pub fn require_optional(&mut self, feature: &str) {
        self.optional.push(feature.to_owned());
    }

    #[must_use]
    pub fn current(&self) -> WireVersion {
        self.current
    }
}

/// Negociação executada antes do efeito.
pub struct Negotiation;

impl Negotiation {
    /// # Errors
    /// `PeerOutsideWindow` antes de qualquer efeito; `MissingCriticalFeature` quando o
    /// peer não oferece todas as features críticas.
    pub fn run(
        profile: &CompatibilityProfile,
        peer: WireVersion,
        offered: &[String],
    ) -> Result<WireVersion, VersionError> {
        if !profile.supports(peer) {
            return Err(VersionError::PeerOutsideWindow);
        }
        for feature in &profile.critical {
            if !offered.iter().any(|f| f == feature) {
                return Err(VersionError::MissingCriticalFeature);
            }
        }
        Ok(peer.min(profile.current))
    }
}

/// Erros de negociação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionError {
    EmptyWindow,
    PeerOutsideWindow,
    MissingCriticalFeature,
}
