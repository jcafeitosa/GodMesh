//! Ciclo administrativo de grants: emitir, consultar e revogar.
//!
//! Fronteira: quem chama já passou pelo portão `admin:grants`. O corpo declara
//! **o quê** (tenant, principal, escopos, TTL), nunca o token nem o hash — os
//! dois são derivados aqui. O token existe em claro apenas na resposta da
//! emissão e não é persistido, auditado ou registrado em log (ADR-026/ADR-031).
//!
//! Revogar grava em `grant_revocation` (migração 0004); o efeito imediato no
//! processo vem da invalidação do cache de revogação, feita pelo chamador HTTP
//! logo depois do commit. Sem reinício, sem reconfiguração.
//!
//! Tenant desconhecido não é recusado: um grant pode preceder a declaração de
//! jurisdição do tenant. Quem recusa operação sem jurisdição é o caso de uso de
//! domínio, não a emissão.

use std::fmt;

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::api::ApiError;
use crate::auth::{generate_credential, token_hash, Scope, ISSUED_AUTHORITY_EPOCH};

/// TTL máximo aceito na emissão: 30 dias.
///
/// Limite desta fatia, declarado e recusado — não truncado em silêncio. Grant de
/// longa duração é decisão de política, não default de rota.
pub const MAX_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

/// Teto de caracteres para `tenant_id` e `principal`. Acima disso o corpo é
/// recusado em vez de gravado.
pub const MAX_IDENTIFIER_CHARS: usize = 256;

/// Teto de caracteres do motivo de revogação.
pub const MAX_REASON_CHARS: usize = 1024;

/// Corpo de `POST /v1/admin/grants`.
///
/// `deny_unknown_fields`: tentar declarar `token`, `token_hash`, `token_id` ou
/// `authority_epoch` no corpo é erro de fronteira, não campo ignorado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueGrantPayload {
    pub tenant_id: String,
    pub principal: String,
    pub scopes: Vec<Scope>,
    pub ttl_seconds: i64,
}

/// Resposta da emissão: o **único** lugar onde o token em claro aparece.
///
/// `Debug` é escrito à mão para redigir o token; um log acidental de
/// `{:?}` não vaza a credencial.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuedGrantResponse {
    pub token_id: String,
    pub token: String,
    pub tenant_id: String,
    pub principal: String,
    pub scopes: Vec<Scope>,
    pub not_before: i64,
    pub expires_at: i64,
    pub authority_epoch: u64,
}

impl fmt::Debug for IssuedGrantResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IssuedGrantResponse")
            .field("token_id", &self.token_id)
            .field("token", &"<redacted>")
            .field("tenant_id", &self.tenant_id)
            .field("principal", &self.principal)
            .field("scopes", &self.scopes)
            .field("not_before", &self.not_before)
            .field("expires_at", &self.expires_at)
            .field("authority_epoch", &self.authority_epoch)
            .finish()
    }
}

/// Metadados de grant: sem token e sem hash, por construção.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantMetadata {
    pub token_id: String,
    pub tenant_id: String,
    pub principal: String,
    pub scopes: Vec<String>,
    pub not_before: i64,
    pub expires_at: i64,
    pub authority_epoch: u64,
    pub created_at: i64,
    pub revoked: bool,
    pub revoked_at: Option<i64>,
    pub revocation_reason: Option<String>,
}

/// Corpo de `POST /v1/admin/grants/{token_id}/revoke`. Motivo é obrigatório.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeGrantPayload {
    pub reason: String,
}

/// Resposta da revogação. Repetir a revogação devolve o mesmo instante: a
/// operação é idempotente e não reescreve a história.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevocationResponse {
    pub token_id: String,
    pub revoked: bool,
    pub revoked_at: i64,
}

fn invalid(detail: impl Into<String>) -> ApiError {
    ApiError::InvalidRequest(detail.into())
}

/// Valida o corpo antes de qualquer efeito.
///
/// # Errors
/// `InvalidRequest` com o motivo exato: identificador vazio/longo, lista de
/// escopos vazia ou repetida, `admin:grants` pedido no corpo (escalada de
/// privilégio por API é recusada: credencial administrativa nasce out-of-band),
/// TTL fora de `1..=MAX_TTL_SECONDS`.
pub fn validate_issue(payload: &IssueGrantPayload) -> Result<(), ApiError> {
    for (label, value) in [
        ("tenant_id", payload.tenant_id.as_str()),
        ("principal", payload.principal.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(invalid(format!("{label} não pode ser vazio")));
        }
        if value.chars().count() > MAX_IDENTIFIER_CHARS {
            return Err(invalid(format!(
                "{label} excede {MAX_IDENTIFIER_CHARS} caracteres"
            )));
        }
    }
    if payload.scopes.is_empty() {
        return Err(invalid("scopes não pode ser vazio"));
    }
    let mut seen: Vec<Scope> = Vec::with_capacity(payload.scopes.len());
    for scope in &payload.scopes {
        if *scope == Scope::AdminGrants {
            return Err(invalid(
                "admin:grants não é emitível por esta rota; credencial administrativa nasce fora da API",
            ));
        }
        if seen.contains(scope) {
            return Err(invalid(format!("escopo repetido: {}", scope.as_str())));
        }
        seen.push(*scope);
    }
    if payload.ttl_seconds <= 0 {
        return Err(invalid("ttl_seconds precisa ser positivo"));
    }
    if payload.ttl_seconds > MAX_TTL_SECONDS {
        return Err(invalid(format!(
            "ttl_seconds excede o máximo desta fatia ({MAX_TTL_SECONDS} s)"
        )));
    }
    Ok(())
}

/// Emite um grant e devolve o token **uma única vez**.
///
/// `now` é o relógio do autorizador, para que emissão e verificação concordem
/// sobre "agora". O banco recebe só o `sha256` hex do token.
///
/// # Errors
/// [`ApiError::InvalidRequest`] de [`validate_issue`]; [`ApiError::Storage`] se
/// o INSERT falhar; falha de entropia vira `Storage` (não há credencial sem RNG).
pub async fn issue_grant(
    pool: &PgPool,
    payload: &IssueGrantPayload,
    now: i64,
) -> Result<IssuedGrantResponse, ApiError> {
    validate_issue(payload)?;
    // Sem entropia não se emite credencial: falha fechada, nunca degradação
    // para um token previsível.
    let (token, token_id) = generate_credential().map_err(ApiError::RandomnessUnavailable)?;
    let not_before = now;
    let expires_at = now
        .checked_add(payload.ttl_seconds)
        .ok_or_else(|| invalid("ttl_seconds transborda o instante de expiração"))?;
    let scopes: Vec<String> = payload
        .scopes
        .iter()
        .map(|scope| scope.as_str().to_owned())
        .collect();

    sqlx::query(
        "INSERT INTO grant_issued \
             (token_id, tenant_id, principal, token_hash, scopes, not_before, expires_at, authority_epoch) \
         VALUES ($1, $2, $3, $4, $5, \
                 to_timestamp($6::double precision), to_timestamp($7::double precision), $8)",
    )
    .bind(&token_id)
    .bind(&payload.tenant_id)
    .bind(&payload.principal)
    .bind(token_hash(&token))
    .bind(&scopes)
    .bind(not_before)
    .bind(expires_at)
    .bind(i64::try_from(ISSUED_AUTHORITY_EPOCH).unwrap_or(i64::MAX))
    .execute(pool)
    .await
    .map_err(ApiError::Storage)?;

    Ok(IssuedGrantResponse {
        token_id,
        token,
        tenant_id: payload.tenant_id.clone(),
        principal: payload.principal.clone(),
        scopes: payload.scopes.clone(),
        not_before,
        expires_at,
        authority_epoch: ISSUED_AUTHORITY_EPOCH,
    })
}

/// Revoga um grant emitido. Idempotente: repetir devolve o mesmo instante.
///
/// # Errors
/// `InvalidRequest` com motivo vazio ou longo demais; `GrantNotFound` para
/// `token_id` que nunca foi emitido (não há revogação de grant inexistente);
/// `Storage` se o banco falhar.
pub async fn revoke_grant(
    pool: &PgPool,
    token_id: &str,
    payload: &RevokeGrantPayload,
) -> Result<RevocationResponse, ApiError> {
    let reason = payload.reason.trim();
    if reason.is_empty() {
        return Err(invalid("reason não pode ser vazio"));
    }
    if reason.chars().count() > MAX_REASON_CHARS {
        return Err(invalid(format!(
            "reason excede {MAX_REASON_CHARS} caracteres"
        )));
    }
    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM grant_issued WHERE token_id = $1")
        .bind(token_id)
        .fetch_optional(pool)
        .await
        .map_err(ApiError::Storage)?;
    if exists.is_none() {
        return Err(ApiError::GrantNotFound);
    }
    // `DO NOTHING`: o instante da primeira revogação não é reescrito. Repetir a
    // operação não apaga a evidência original.
    sqlx::query(
        "INSERT INTO grant_revocation (token_id, reason) VALUES ($1, $2) \
         ON CONFLICT (token_id) DO NOTHING",
    )
    .bind(token_id)
    .bind(reason)
    .execute(pool)
    .await
    .map_err(ApiError::Storage)?;

    let revoked_at: Option<i64> = sqlx::query_scalar(
        "SELECT (EXTRACT(EPOCH FROM revoked_at))::bigint \
           FROM grant_revocation WHERE token_id = $1",
    )
    .bind(token_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::Storage)?;

    let revoked_at = revoked_at.ok_or(ApiError::GrantNotFound)?;
    Ok(RevocationResponse {
        token_id: token_id.to_owned(),
        revoked: true,
        revoked_at,
    })
}

/// Metadados de um grant emitido, sem token e sem hash.
///
/// # Errors
/// [`ApiError::GrantNotFound`] para `token_id` desconhecido; `Storage` se o
/// banco falhar. A consulta não seleciona `token_hash`: o hash não tem como
/// sair por aqui nem por engano.
pub async fn grant_metadata(pool: &PgPool, token_id: &str) -> Result<GrantMetadata, ApiError> {
    let row = sqlx::query(
        "SELECT g.token_id, g.tenant_id, g.principal, g.scopes, \
                (EXTRACT(EPOCH FROM g.not_before))::bigint AS not_before, \
                (EXTRACT(EPOCH FROM g.expires_at))::bigint AS expires_at, \
                g.authority_epoch, \
                (EXTRACT(EPOCH FROM g.created_at))::bigint AS created_at, \
                (EXTRACT(EPOCH FROM r.revoked_at))::bigint AS revoked_at, \
                r.reason AS revocation_reason \
           FROM grant_issued g \
           LEFT JOIN grant_revocation r ON r.token_id = g.token_id \
          WHERE g.token_id = $1",
    )
    .bind(token_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::Storage)?
    .ok_or(ApiError::GrantNotFound)?;

    let revoked_at: Option<i64> = row.get("revoked_at");
    let epoch: i64 = row.get("authority_epoch");
    Ok(GrantMetadata {
        token_id: row.get("token_id"),
        tenant_id: row.get("tenant_id"),
        principal: row.get("principal"),
        scopes: row.get("scopes"),
        not_before: row.get("not_before"),
        expires_at: row.get("expires_at"),
        authority_epoch: u64::try_from(epoch).unwrap_or(0),
        created_at: row.get("created_at"),
        revoked: revoked_at.is_some(),
        revoked_at,
        revocation_reason: row.get("revocation_reason"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(scopes: Vec<Scope>, ttl_seconds: i64) -> IssueGrantPayload {
        IssueGrantPayload {
            tenant_id: "tenant-a".to_owned(),
            principal: "p1".to_owned(),
            scopes,
            ttl_seconds,
        }
    }

    #[test]
    fn rejects_empty_identifiers_and_bad_ttl() {
        let mut p = payload(vec![Scope::IncidentRead], 60);
        p.tenant_id = "  ".to_owned();
        assert!(validate_issue(&p).is_err());

        let mut p = payload(vec![Scope::IncidentRead], 60);
        p.principal = String::new();
        assert!(validate_issue(&p).is_err());

        assert!(validate_issue(&payload(vec![Scope::IncidentRead], 0)).is_err());
        assert!(validate_issue(&payload(vec![Scope::IncidentRead], -1)).is_err());
        assert!(validate_issue(&payload(vec![Scope::IncidentRead], MAX_TTL_SECONDS + 1)).is_err());
        assert!(validate_issue(&payload(vec![Scope::IncidentRead], MAX_TTL_SECONDS)).is_ok());
    }

    #[test]
    fn rejects_empty_duplicate_and_admin_scopes() {
        assert!(validate_issue(&payload(Vec::new(), 60)).is_err());
        assert!(
            validate_issue(&payload(vec![Scope::IncidentRead, Scope::IncidentRead], 60)).is_err()
        );
        assert!(validate_issue(&payload(vec![Scope::AdminGrants], 60)).is_err());
        assert!(
            validate_issue(&payload(
                vec![Scope::IncidentRead, Scope::IncidentWrite],
                60
            ))
            .is_ok()
        );
    }

    #[test]
    fn response_debug_redacts_the_token() {
        let response = IssuedGrantResponse {
            token_id: "gtr_x".to_owned(),
            token: "gm_segredo".to_owned(),
            tenant_id: "t".to_owned(),
            principal: "p".to_owned(),
            scopes: vec![Scope::IncidentRead],
            not_before: 1,
            expires_at: 2,
            authority_epoch: 1,
        };
        let rendered = format!("{response:?}");
        assert!(!rendered.contains("gm_segredo"), "{rendered}");
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn metadata_has_no_token_or_hash_fields() {
        let metadata = GrantMetadata {
            token_id: "gtr_x".to_owned(),
            tenant_id: "t".to_owned(),
            principal: "p".to_owned(),
            scopes: vec!["incident:read".to_owned()],
            not_before: 1,
            expires_at: 2,
            authority_epoch: 1,
            created_at: 1,
            revoked: false,
            revoked_at: None,
            revocation_reason: None,
        };
        let json = serde_json::to_value(&metadata).expect("serializa");
        assert!(json.get("token").is_none());
        assert!(json.get("token_hash").is_none());
    }
}