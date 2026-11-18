//! Fronteira de autenticação: credencial apresentada → grant verificado.
//!
//! Regra da fatia: **nenhum campo de autorização ou de tenancy vem do corpo**.
//! O `Authorization: Bearer <grant>` é resolvido contra o registro de grants
//! verificados; o `GrantPredicate` e o sujeito (`tenant_id`, `principal`) saem
//! daí. O formato do token é opaco nesta fatia — não há claim, assinatura nem
//! introspecção implementadas aqui, e nada nesta fatia deve ser lido como prova
//! de posse. Registro ausente ou token desconhecido nunca autoriza.
//!
//! Segredos: o token nunca aparece em `Debug`, em erro ou em log. O `Debug` do
//! registro é escrito à mão para redigir o token.

use std::fmt;

use godmesh_identity::grant::{
    FreshnessSource, GrantPredicate, OfflineOp, RevocationState,
};
use serde::Deserialize;

/// Sujeito autenticado: a única fonte de tenancy aceita pelo servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    pub tenant_id: String,
    pub principal: String,
}

/// Grant verificado: sujeito + predicado do ADR-059/ADR-064.
///
/// Só é construído a partir do registro de grants, nunca do corpo do pedido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedGrant {
    pub subject: Subject,
    pub predicate: GrantPredicate,
}

/// Motivo de recusa de uma credencial estruturalmente inválida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    /// Token vazio não identifica ninguém.
    EmptyToken,
    /// Sem tenant autenticado não há tenancy a derivar.
    EmptyTenant,
    /// Sem principal autenticado não há autoria a atribuir.
    EmptyPrincipal,
}

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self {
            Self::EmptyToken => "token vazio",
            Self::EmptyTenant => "grant sem tenant autenticado",
            Self::EmptyPrincipal => "grant sem principal autenticado",
        };
        f.write_str(reason)
    }
}

impl VerifiedGrant {
    /// Recusa grant sem sujeito completo. Roda antes de qualquer efeito.
    ///
    /// # Errors
    /// `EmptyTenant`/`EmptyPrincipal` quando o sujeito verificado está incompleto.
    pub fn validate(&self) -> Result<(), CredentialError> {
        if self.subject.tenant_id.trim().is_empty() {
            return Err(CredentialError::EmptyTenant);
        }
        if self.subject.principal.trim().is_empty() {
            return Err(CredentialError::EmptyPrincipal);
        }
        Ok(())
    }
}

/// Entrada do registro: token opaco → grant verificado.
#[derive(Clone)]
struct RegistryEntry {
    token: String,
    grant: VerifiedGrant,
}

impl fmt::Debug for RegistryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // O token é segredo: `Debug` mostra apenas que existe.
        f.debug_struct("RegistryEntry")
            .field("token", &"<redacted>")
            .field("grant", &self.grant)
            .finish()
    }
}

/// Registro de grants verificados consultado pelo servidor.
///
/// Nesta fatia é configurado por ambiente. Verificação criptográfica, emissão e
/// revogação de credenciais continuam pendentes de decisão (ADR de identidade).
#[derive(Debug, Clone, Default)]
pub struct GrantRegistry {
    entries: Vec<RegistryEntry>,
}

/// Forma serializada de uma entrada de configuração.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGrant {
    token: String,
    tenant_id: String,
    principal: String,
    operation: OfflineOp,
    freshness: FreshnessSource,
    revocation: RevocationState,
    epoch_comparable: bool,
    action_enumerated: bool,
    retention_class_known: bool,
}

/// Forma serializada do registro.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRegistry {
    grants: Vec<RawGrant>,
}

/// Variável de ambiente com o registro de grants (JSON).
pub const GRANTS_ENV: &str = "GODMESH_GRANTS";

impl GrantRegistry {
    /// Lê o registro da variável de ambiente `GODMESH_GRANTS`.
    ///
    /// # Errors
    /// Variável ausente, vazia, não-UTF8 ou JSON inválido. A mensagem nunca
    /// inclui o conteúdo (que contém tokens).
    pub fn from_env() -> Result<Self, String> {
        let raw = match std::env::var(GRANTS_ENV) {
            Ok(v) if !v.trim().is_empty() => v,
            Ok(_) => {
                return Err(format!(
                    "{GRANTS_ENV} está definida mas vazia; sem grants o endpoint de incidente nega tudo"
                ));
            }
            Err(std::env::VarError::NotPresent) => {
                return Err(format!(
                    "{GRANTS_ENV} não definida; exemplo: {GRANTS_ENV}='{{\"grants\":[...]}}'"
                ));
            }
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(format!("{GRANTS_ENV} contém bytes não-UTF8"));
            }
        };
        Self::from_json(&raw).map_err(|e| format!("{GRANTS_ENV} inválida: {e}"))
    }

    /// Constrói o registro a partir do JSON de configuração.
    ///
    /// # Errors
    /// JSON malformado, campo desconhecido, token vazio ou token duplicado.
    pub fn from_json(raw: &str) -> Result<Self, String> {
        let parsed: RawRegistry =
            serde_json::from_str(raw).map_err(|e| format!("JSON malformado: {e}"))?;
        let mut entries: Vec<RegistryEntry> = Vec::with_capacity(parsed.grants.len());
        for item in parsed.grants {
            let grant = VerifiedGrant {
                subject: Subject {
                    tenant_id: item.tenant_id,
                    principal: item.principal,
                },
                predicate: GrantPredicate {
                    operation: item.operation,
                    epoch_comparable: item.epoch_comparable,
                    revocation: item.revocation,
                    freshness: item.freshness,
                    retention_class_known: item.retention_class_known,
                    action_enumerated: item.action_enumerated,
                },
            };
            if item.token.is_empty() {
                return Err(CredentialError::EmptyToken.to_string());
            }
            if entries.iter().any(|e| e.token == item.token) {
                return Err("token duplicado no registro de grants".to_owned());
            }
            entries.push(RegistryEntry {
                token: item.token,
                grant,
            });
        }
        Ok(Self { entries })
    }

    /// Constrói o registro a partir de pares (token, grant). Usado por testes e
    /// por composição em memória.
    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = (String, VerifiedGrant)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(token, grant)| RegistryEntry { token, grant })
                .collect(),
        }
    }

    /// Quantidade de grants configurados. Não revela quais.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` quando nenhum grant está configurado (tudo nega).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Resolve o token apresentado; `None` quando não corresponde a nenhum grant.
    ///
    /// A varredura não para no primeiro acerto e compara em tempo constante por
    /// entrada, para não transformar prefixo/posição em oráculo de timing.
    #[must_use]
    pub fn verify(&self, token: &str) -> Option<&VerifiedGrant> {
        let probe = token.as_bytes();
        let mut found: Option<&VerifiedGrant> = None;
        for entry in &self.entries {
            let candidate = entry.token.as_bytes();
            let equal = candidate.len() == probe.len()
                && candidate
                    .iter()
                    .zip(probe)
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    == 0;
            if equal && found.is_none() {
                found = Some(&entry.grant);
            }
        }
        found
    }
}

/// Extrai o token de `Authorization: Bearer <token>` (RFC 7235: esquema
/// case-insensitive, um único espaço, token não vazio).
#[must_use]
pub fn bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    let raw = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = raw.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = token.trim();
    if token.is_empty() || token.contains(' ') {
        return None;
    }
    Some(token)
}
