//! Fronteira de autenticação e autorização: credencial apresentada → grant
//! verificado → escopo exigido pela rota.
//!
//! Regra da fatia: **nenhum campo de autorização ou de tenancy vem do corpo**.
//! O `Authorization: Bearer <token>` é resolvido contra o registro de grants
//! verificados; sujeito (`tenant_id`, `principal`), escopos e predicado saem daí.
//! O token continua **opaco**: nenhuma cifra, assinatura ou JWT é inventada aqui
//! (ADR-026/ADR-031). A credencial é uma referência de banco de dados, e a
//! revogação vive em banco, não no processo.
//!
//! Modelo do grant: além do sujeito e do predicado do ADR-059/ADR-064, um grant
//! carrega `token_id`, escopos, janela de validade (`not_before`/`expires_at`),
//! revogação declarada (`revoked_at`) e `authority_epoch`.
//!
//! Segredos: o token nunca aparece em `Debug`, em erro, em auditoria ou em log.
//! O `Debug` do registro é escrito à mão para redigir o token, e a auditoria
//! externa só recebe `token_id` e o motivo.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use godmesh_identity::grant::{
    FreshnessSource, GrantPredicate, OfflineOp, RevocationState,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::Mutex;

/// Sujeito autenticado: a única fonte de tenancy aceita pelo servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subject {
    pub tenant_id: String,
    pub principal: String,
}

/// Escopo exigido por uma rota. A rota declara um destes; o grant precisa
/// contê-lo, senão a resposta é `403 insufficient_scope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    #[serde(rename = "incident:write")]
    IncidentWrite,
    #[serde(rename = "incident:read")]
    IncidentRead,
    #[serde(rename = "event:write")]
    EventWrite,
    #[serde(rename = "event:read")]
    EventRead,
    #[serde(rename = "sync:write")]
    SyncWrite,
    #[serde(rename = "sync:read")]
    SyncRead,
}

impl Scope {
    /// Conjunto completo, usado apenas por composição de teste (`from_entries`).
    pub const ALL: [Self; 6] = [
        Self::IncidentWrite,
        Self::IncidentRead,
        Self::EventWrite,
        Self::EventRead,
        Self::SyncWrite,
        Self::SyncRead,
    ];

    /// Nome estável do escopo no contrato JSON e nos corpos de erro.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IncidentWrite => "incident:write",
            Self::IncidentRead => "incident:read",
            Self::EventWrite => "event:write",
            Self::EventRead => "event:read",
            Self::SyncWrite => "sync:write",
            Self::SyncRead => "sync:read",
        }
    }
}

/// Um grant como declarado no registro: identidade, autorização e janela.
///
/// `revoked_at` aqui é a revogação **declarada na configuração**; a revogação
/// operacional fica na tabela `grant_revocation` e é consultada em tempo de
/// requisição (ver [`Authorization`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantRecord {
    pub token_id: String,
    pub subject: Subject,
    pub scopes: Vec<Scope>,
    pub not_before: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    pub authority_epoch: u64,
    pub predicate: GrantPredicate,
}

impl GrantRecord {
    /// `true` quando o grant contém o escopo exigido.
    #[must_use]
    pub fn has_scope(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope)
    }

    /// Materializa a credencial aceita, sem os campos de janela.
    #[must_use]
    pub fn verified(&self) -> VerifiedGrant {
        VerifiedGrant {
            subject: self.subject.clone(),
            predicate: self.predicate,
            token_id: self.token_id.clone(),
            scopes: self.scopes.clone(),
            authority_epoch: self.authority_epoch,
        }
    }
}

/// Grant verificado entregue ao handler: sujeito, escopos e predicado.
///
/// Só é construído a partir de um [`GrantRecord`] que passou por janela,
/// revogação e escopo, nunca do corpo do pedido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedGrant {
    pub subject: Subject,
    pub predicate: GrantPredicate,
    pub token_id: String,
    pub scopes: Vec<Scope>,
    pub authority_epoch: u64,
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

    /// `true` quando o grant verificado contém o escopo.
    #[must_use]
    pub fn has_scope(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope)
    }
}

/// Recusa de autorização com código de erro estável. Nenhuma variante carrega o
/// token apresentado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthError {
    /// Token ausente ou fora do registro.
    Unauthenticated,
    /// `expires_at` já passou.
    CredentialExpired,
    /// `not_before` ainda não chegou.
    CredentialNotYetValid,
    /// Revogação declarada na configuração ou persistida em `grant_revocation`.
    CredentialRevoked,
    /// Grant válido, mas sem o escopo que a rota exige.
    InsufficientScope,
    /// Não foi possível consultar a revogação: nega (falha fechada).
    RevocationUnavailable,
    /// Sujeito verificado incompleto.
    InvalidCredential(CredentialError),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self {
            Self::Unauthenticated => "credencial ausente ou desconhecida",
            Self::CredentialExpired => "credencial expirada",
            Self::CredentialNotYetValid => "credencial ainda não válida",
            Self::CredentialRevoked => "credencial revogada",
            Self::InsufficientScope => "escopo insuficiente",
            Self::RevocationUnavailable => "revogação indisponível; acesso negado",
            Self::InvalidCredential(e) => return e.fmt(f),
        };
        f.write_str(reason)
    }
}

impl std::error::Error for AuthError {}

/// Relógio injetável: a janela do grant é avaliada contra esta fonte, não contra
/// `SystemTime` espalhado pelo código.
pub trait Clock: Send + Sync + fmt::Debug {
    /// Instante corrente em segundos desde a época Unix.
    fn now_unix(&self) -> i64;
}

/// Relógio de produção.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
    }
}

/// Relógio fixo para teste determinístico.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub i64);

impl Clock for FixedClock {
    fn now_unix(&self) -> i64 {
        self.0
    }
}

/// Resultado assíncrono da consulta de revogação, sem dependência extra.
pub type RevocationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<i64>, String>> + Send + 'a>>;

/// Fonte de revogação persistente. A produção consulta o banco; os testes podem
/// injetar uma fonte em memória e provar que revogar não exige reinício.
pub trait RevocationSource: Send + Sync + fmt::Debug {
    /// Instante de revogação em segundos Unix, ou `None` quando não revogado.
    fn revoked_at<'a>(&'a self, token_id: &'a str) -> RevocationFuture<'a>;
}

/// Revogação persistida em Postgres (`grant_revocation`).
#[derive(Debug, Clone)]
pub struct PgRevocationSource {
    pool: PgPool,
}

impl PgRevocationSource {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl RevocationSource for PgRevocationSource {
    fn revoked_at<'a>(&'a self, token_id: &'a str) -> RevocationFuture<'a> {
        Box::pin(async move {
            // `EXTRACT` evita habilitar features de data no sqlx; o valor não
            // decodifica como `timestamptz`, então a conversão é explícita.
            let row: Option<Option<i64>> = sqlx::query_scalar(
                "SELECT (EXTRACT(EPOCH FROM revoked_at))::bigint \
                   FROM grant_revocation WHERE token_id = $1",
            )
            .bind(token_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| "falha ao consultar grant_revocation".to_owned())?;
            Ok(row.flatten())
        })
    }
}

/// TTL documentado do cache de revogação. Curto de propósito: revogar propaga em
/// no máximo 5 s por processo, e [`Authorization::invalidate_revocation`] permite
/// invalidar explicitamente quando o operador quer efeito imediato.
pub const REVOCATION_CACHE_TTL_SECONDS: i64 = 5;

#[derive(Debug, Clone)]
struct CachedRevocation {
    revoked_at: Option<i64>,
    fetched_at: i64,
}

/// Cache em memória da revogação, com TTL e invalidação explícita.
#[derive(Debug, Clone)]
pub struct RevocationCache {
    ttl_seconds: i64,
    inner: Arc<Mutex<HashMap<String, CachedRevocation>>>,
}

impl RevocationCache {
    #[must_use]
    pub fn new(ttl_seconds: i64) -> Self {
        Self {
            ttl_seconds,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn fresh(&self, token_id: &str, now: i64) -> Option<Option<i64>> {
        let guard = self.inner.lock().await;
        let slot = guard.get(token_id)?;
        // Estritamente menor: TTL 0 desliga a reutilização.
        if now.saturating_sub(slot.fetched_at) < self.ttl_seconds {
            Some(slot.revoked_at)
        } else {
            None
        }
    }

    async fn store(&self, token_id: &str, revoked_at: Option<i64>, now: i64) {
        self.inner.lock().await.insert(
            token_id.to_owned(),
            CachedRevocation {
                revoked_at,
                fetched_at: now,
            },
        );
    }

    /// Invalidação explícita de um `token_id`.
    pub async fn invalidate(&self, token_id: &str) {
        self.inner.lock().await.remove(token_id);
    }

    /// Invalidação explícita de todo o cache.
    pub async fn invalidate_all(&self) {
        self.inner.lock().await.clear();
    }
}

impl Default for RevocationCache {
    fn default() -> Self {
        Self::new(REVOCATION_CACHE_TTL_SECONDS)
    }
}

/// Destino da auditoria de autenticação. Recebe só `token_id` e motivo; nunca o
/// token.
pub trait AuditSink: Send + Sync + fmt::Debug {
    fn record(&self, event: &str);
}

/// Auditoria de produção: uma linha em stderr.
#[derive(Debug, Clone, Copy, Default)]
pub struct StderrAudit;

impl AuditSink for StderrAudit {
    fn record(&self, event: &str) {
        eprintln!("godmesh-server: auth {event}");
    }
}

/// Auditoria em memória, para provar em teste que nenhum token é registrado.
#[derive(Debug, Default)]
pub struct CapturedAudit {
    lines: Mutex<Vec<String>>,
}

impl CapturedAudit {
    /// Linhas registradas até agora.
    pub async fn lines(&self) -> Vec<String> {
        self.lines.lock().await.clone()
    }
}

impl AuditSink for CapturedAudit {
    fn record(&self, event: &str) {
        // `record` é síncrono por contrato do middleware; `try_lock` é seguro
        // porque só há um escritor por requisição e o teste não disputa.
        if let Ok(mut guard) = self.lines.try_lock() {
            guard.push(event.to_owned());
        }
    }
}

/// Entrada do registro: token opaco → grant declarado.
#[derive(Clone)]
struct RegistryEntry {
    token: String,
    record: GrantRecord,
}

impl fmt::Debug for RegistryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // O token é segredo: `Debug` mostra apenas que existe.
        f.debug_struct("RegistryEntry")
            .field("token", &"<redacted>")
            .field("record", &self.record)
            .finish()
    }
}

/// Registro de grants verificados consultado pelo servidor.
#[derive(Debug, Clone, Default)]
pub struct GrantRegistry {
    entries: Vec<RegistryEntry>,
}

/// Forma serializada de uma entrada de configuração.
///
/// `not_before`, `expires_at` e `revoked_at` são segundos desde a época Unix.
/// `expires_at` é obrigatório: grant sem prazo é grant eterno, e esta fatia não
/// aceita isso silenciosamente.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGrant {
    token: String,
    token_id: String,
    tenant_id: String,
    principal: String,
    scopes: Vec<Scope>,
    not_before: i64,
    expires_at: i64,
    #[serde(default)]
    revoked_at: Option<i64>,
    authority_epoch: u64,
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
    /// JSON malformado, campo desconhecido, token/`token_id` vazio ou duplicado,
    /// lista de escopos vazia ou repetida, janela inválida (`expires_at` não é
    /// posterior a `not_before`), instantes negativos.
    pub fn from_json(raw: &str) -> Result<Self, String> {
        let parsed: RawRegistry =
            serde_json::from_str(raw).map_err(|e| format!("JSON malformado: {e}"))?;
        let mut entries: Vec<RegistryEntry> = Vec::with_capacity(parsed.grants.len());
        for item in parsed.grants {
            if item.token.is_empty() {
                return Err(CredentialError::EmptyToken.to_string());
            }
            if item.token_id.trim().is_empty() {
                return Err("grant sem token_id".to_owned());
            }
            if item.scopes.is_empty() {
                return Err(format!("grant {} sem escopo declarado", item.token_id));
            }
            if item.not_before < 0 || item.expires_at < 0 {
                return Err(format!(
                    "grant {} tem instante negativo; use segundos desde a época Unix",
                    item.token_id
                ));
            }
            if item.expires_at <= item.not_before {
                return Err(format!(
                    "grant {} tem expires_at <= not_before; a janela precisa ser positiva",
                    item.token_id
                ));
            }
            let mut scopes = Vec::with_capacity(item.scopes.len());
            for scope in item.scopes {
                if scopes.contains(&scope) {
                    return Err(format!(
                        "grant {} repete o escopo {}",
                        item.token_id,
                        scope.as_str()
                    ));
                }
                scopes.push(scope);
            }
            if entries.iter().any(|e| e.token == item.token) {
                return Err("token duplicado no registro de grants".to_owned());
            }
            if entries.iter().any(|e| e.record.token_id == item.token_id) {
                return Err(format!("token_id duplicado no registro: {}", item.token_id));
            }
            entries.push(RegistryEntry {
                token: item.token,
                record: GrantRecord {
                    token_id: item.token_id,
                    subject: Subject {
                        tenant_id: item.tenant_id,
                        principal: item.principal,
                    },
                    scopes,
                    not_before: item.not_before,
                    expires_at: item.expires_at,
                    revoked_at: item.revoked_at,
                    authority_epoch: item.authority_epoch,
                    predicate: GrantPredicate {
                        operation: item.operation,
                        epoch_comparable: item.epoch_comparable,
                        revocation: item.revocation,
                        freshness: item.freshness,
                        retention_class_known: item.retention_class_known,
                        action_enumerated: item.action_enumerated,
                    },
                },
            });
        }
        Ok(Self { entries })
    }

    /// Constrói o registro a partir de pares (token, grant já verificado).
    ///
    /// Composição de teste/composição em memória: o grant já passou pela
    /// autorização, então recebe todos os escopos, janela aberta e `token_id`
    /// posicional. Caminho de produção é [`Self::from_env`].
    #[must_use]
    pub fn from_entries(entries: impl IntoIterator<Item = (String, VerifiedGrant)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(token, grant)| RegistryEntry {
                    token,
                    record: GrantRecord {
                        token_id: grant.token_id,
                        subject: grant.subject,
                        scopes: if grant.scopes.is_empty() {
                            Scope::ALL.to_vec()
                        } else {
                            grant.scopes
                        },
                        not_before: 0,
                        expires_at: i64::MAX,
                        revoked_at: None,
                        authority_epoch: grant.authority_epoch,
                        predicate: grant.predicate,
                    },
                })
                .collect(),
        }
    }

    /// Constrói o registro a partir de registros completos (com token). Usado
    /// por testes que precisam da janela e dos escopos declarados.
    #[must_use]
    pub fn from_records(entries: impl IntoIterator<Item = (String, GrantRecord)>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .map(|(token, record)| RegistryEntry { token, record })
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
    pub fn verify(&self, token: &str) -> Option<&GrantRecord> {
        let probe = token.as_bytes();
        let mut found: Option<&GrantRecord> = None;
        for entry in &self.entries {
            let candidate = entry.token.as_bytes();
            let equal = candidate.len() == probe.len()
                && candidate
                    .iter()
                    .zip(probe)
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    == 0;
            if equal && found.is_none() {
                found = Some(&entry.record);
            }
        }
        found
    }
}

/// Autorizador: registro + relógio + fonte de revogação + cache + auditoria.
///
/// Uma requisição só chega ao handler depois de [`Self::authenticate`] aceitar a
/// credencial e do portão de escopo da rota aceitar o escopo. A revogação é
/// consultada em banco (com cache curto), então revogar não exige reiniciar o
/// processo.
#[derive(Debug, Clone)]
pub struct Authorization {
    registry: GrantRegistry,
    clock: Arc<dyn Clock>,
    revocation: Arc<dyn RevocationSource>,
    cache: RevocationCache,
    audit: Arc<dyn AuditSink>,
}

impl Authorization {
    /// Composição de produção: relógio do sistema, revogação em Postgres, TTL
    /// documentado de [`REVOCATION_CACHE_TTL_SECONDS`] e auditoria em stderr.
    #[must_use]
    pub fn new(registry: GrantRegistry, pool: PgPool) -> Self {
        Self {
            registry,
            clock: Arc::new(SystemClock),
            revocation: Arc::new(PgRevocationSource::new(pool)),
            cache: RevocationCache::default(),
            audit: Arc::new(StderrAudit),
        }
    }

    /// Composição explícita, usada por testes e por quem injeta outra fonte.
    pub fn with_parts(
        registry: GrantRegistry,
        clock: Arc<dyn Clock>,
        revocation: Arc<dyn RevocationSource>,
        cache_ttl_seconds: i64,
        audit: Arc<dyn AuditSink>,
    ) -> Self {
        Self {
            registry,
            clock,
            revocation,
            cache: RevocationCache::new(cache_ttl_seconds),
            audit,
        }
    }

    /// Registro consultado. Exposto para inspeção de contagem, nunca de token.
    #[must_use]
    pub const fn registry(&self) -> &GrantRegistry {
        &self.registry
    }

    /// Invalida explicitamente o cache de revogação de um `token_id`; com isso a
    /// próxima requisição relê o banco sem esperar o TTL.
    pub async fn invalidate_revocation(&self, token_id: &str) {
        self.cache.invalidate(token_id).await;
    }

    /// Invalida explicitamente todo o cache de revogação.
    pub async fn invalidate_all_revocations(&self) {
        self.cache.invalidate_all().await;
    }

    async fn revocation_status(
        &self,
        token_id: &str,
        now: i64,
    ) -> Result<Option<i64>, AuthError> {
        if let Some(cached) = self.cache.fresh(token_id, now).await {
            return Ok(cached);
        }
        match self.revocation.revoked_at(token_id).await {
            Ok(revoked_at) => {
                self.cache.store(token_id, revoked_at, now).await;
                Ok(revoked_at)
            }
            Err(_) => Err(AuthError::RevocationUnavailable),
        }
    }

    /// Autentica a credencial sem julgar escopo.
    ///
    /// Ordem deliberada: token desconhecido → revogação declarada → janela →
    /// revogação persistida → sujeito. O escopo é julgado por [`Self::authorize`]
    /// (ou pelo portão da rota), porque o portão de autenticação não conhece a
    /// rota.
    ///
    /// # Errors
    /// [`AuthError`] com o motivo exato da recusa. Nenhuma mensagem contém o token.
    pub async fn authenticate(&self, token: &str) -> Result<VerifiedGrant, AuthError> {
        let Some(record) = self.registry.verify(token) else {
            self.audit.record("event=auth_denied reason=unauthenticated");
            return Err(AuthError::Unauthenticated);
        };
        let token_id = record.token_id.as_str();
        if record.revoked_at.is_some() {
            self.audit
                .record(&format!("event=auth_denied token_id={token_id} reason=credential_revoked"));
            return Err(AuthError::CredentialRevoked);
        }
        let now = self.clock.now_unix();
        if now < record.not_before {
            self.audit.record(&format!(
                "event=auth_denied token_id={token_id} reason=credential_not_yet_valid"
            ));
            return Err(AuthError::CredentialNotYetValid);
        }
        if now >= record.expires_at {
            self.audit.record(&format!(
                "event=auth_denied token_id={token_id} reason=credential_expired"
            ));
            return Err(AuthError::CredentialExpired);
        }
        if self.revocation_status(token_id, now).await?.is_some() {
            self.audit
                .record(&format!("event=auth_denied token_id={token_id} reason=credential_revoked"));
            return Err(AuthError::CredentialRevoked);
        }
        let verified = record.verified();
        if let Err(e) = verified.validate() {
            self.audit.record(&format!(
                "event=auth_denied token_id={token_id} reason=invalid_credential"
            ));
            return Err(AuthError::InvalidCredential(e));
        }
        self.audit
            .record(&format!("event=auth_accepted token_id={token_id}"));
        Ok(verified)
    }

    /// Autentica e exige o escopo da rota.
    ///
    /// # Errors
    /// Os mesmos de [`Self::authenticate`], mais [`AuthError::InsufficientScope`].
    pub async fn authorize(
        &self,
        token: &str,
        required: Scope,
    ) -> Result<VerifiedGrant, AuthError> {
        let verified = self.authenticate(token).await?;
        if !verified.has_scope(required) {
            self.audit.record(&format!(
                "event=auth_denied token_id={} reason=insufficient_scope scope={}",
                verified.token_id,
                required.as_str()
            ));
            return Err(AuthError::InsufficientScope);
        }
        Ok(verified)
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

/// Conversão de [`Duration`] para o TTL em segundos do cache (arredonda para
/// baixo; mínimo zero). Exposta para composição de teste e operação.
#[must_use]
pub fn cache_ttl_seconds(ttl: Duration) -> i64 {
    i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    fn predicate() -> GrantPredicate {
        GrantPredicate {
            operation: OfflineOp::OpenSosIncident,
            epoch_comparable: true,
            revocation: RevocationState::Absent,
            freshness: FreshnessSource::F3VerifiedOnline,
            retention_class_known: true,
            action_enumerated: true,
        }
    }

    fn record(scopes: Vec<Scope>) -> GrantRecord {
        GrantRecord {
            token_id: "grant-1".to_owned(),
            subject: Subject {
                tenant_id: "tenant-a".to_owned(),
                principal: "p1".to_owned(),
            },
            scopes,
            not_before: 100,
            expires_at: 200,
            revoked_at: None,
            authority_epoch: 7,
            predicate: predicate(),
        }
    }

    #[derive(Debug, Default)]
    struct MemoryRevocation {
        revoked: Mutex<Option<i64>>,
    }

    /// Relógio mutável: o teste avança o tempo para atravessar o TTL do cache.
    #[derive(Debug)]
    struct StepClock(AtomicI64);

    impl StepClock {
        fn new(now: i64) -> Self {
            Self(AtomicI64::new(now))
        }

        fn set(&self, now: i64) {
            self.0.store(now, Ordering::SeqCst);
        }
    }

    impl Clock for StepClock {
        fn now_unix(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    impl RevocationSource for MemoryRevocation {
        fn revoked_at<'a>(&'a self, _token_id: &'a str) -> RevocationFuture<'a> {
            Box::pin(async move { Ok(*self.revoked.lock().await) })
        }
    }

    fn authorization(
        registry: GrantRegistry,
        now: i64,
        revocation: Arc<MemoryRevocation>,
        audit: Arc<CapturedAudit>,
    ) -> Authorization {
        Authorization::with_parts(
            registry,
            Arc::new(FixedClock(now)),
            revocation,
            REVOCATION_CACHE_TTL_SECONDS,
            audit,
        )
    }

    #[tokio::test]
    async fn valid_grant_is_accepted_within_window() {
        let registry = GrantRegistry::from_records([(
            "token-secreto".to_owned(),
            record(vec![Scope::IncidentWrite]),
        )]);
        let auth = authorization(
            registry,
            150,
            Arc::new(MemoryRevocation::default()),
            Arc::new(CapturedAudit::default()),
        );
        let verified = auth
            .authorize("token-secreto", Scope::IncidentWrite)
            .await
            .expect("grant válido");
        assert_eq!(verified.subject.tenant_id, "tenant-a");
        assert_eq!(verified.token_id, "grant-1");
        assert_eq!(verified.authority_epoch, 7);
    }

    #[tokio::test]
    async fn expired_grant_is_credential_expired() {
        let registry = GrantRegistry::from_records([(
            "t".to_owned(),
            record(vec![Scope::IncidentWrite]),
        )]);
        let auth = authorization(
            registry,
            200,
            Arc::new(MemoryRevocation::default()),
            Arc::new(CapturedAudit::default()),
        );
        assert_eq!(
            auth.authorize("t", Scope::IncidentWrite).await,
            Err(AuthError::CredentialExpired)
        );
    }

    #[tokio::test]
    async fn future_not_before_is_credential_not_yet_valid() {
        let registry = GrantRegistry::from_records([(
            "t".to_owned(),
            record(vec![Scope::IncidentWrite]),
        )]);
        let auth = authorization(
            registry,
            99,
            Arc::new(MemoryRevocation::default()),
            Arc::new(CapturedAudit::default()),
        );
        assert_eq!(
            auth.authorize("t", Scope::IncidentWrite).await,
            Err(AuthError::CredentialNotYetValid)
        );
    }

    #[tokio::test]
    async fn configured_revocation_is_credential_revoked() {
        let mut r = record(vec![Scope::IncidentWrite]);
        r.revoked_at = Some(120);
        let registry = GrantRegistry::from_records([("t".to_owned(), r)]);
        let auth = authorization(
            registry,
            150,
            Arc::new(MemoryRevocation::default()),
            Arc::new(CapturedAudit::default()),
        );
        assert_eq!(
            auth.authorize("t", Scope::IncidentWrite).await,
            Err(AuthError::CredentialRevoked)
        );
    }

    #[tokio::test]
    async fn missing_scope_is_insufficient_scope() {
        let registry = GrantRegistry::from_records([(
            "t".to_owned(),
            record(vec![Scope::IncidentRead]),
        )]);
        let auth = authorization(
            registry,
            150,
            Arc::new(MemoryRevocation::default()),
            Arc::new(CapturedAudit::default()),
        );
        assert_eq!(
            auth.authorize("t", Scope::IncidentWrite).await,
            Err(AuthError::InsufficientScope)
        );
    }

    #[tokio::test]
    async fn database_revocation_takes_effect_without_restart_after_ttl() {
        let registry = GrantRegistry::from_records([(
            "t".to_owned(),
            record(vec![Scope::IncidentWrite]),
        )]);
        let revocation = Arc::new(MemoryRevocation::default());
        let clock = Arc::new(StepClock::new(150));
        let auth = Authorization::with_parts(
            registry,
            clock.clone(),
            revocation.clone(),
            5,
            Arc::new(CapturedAudit::default()),
        );
        auth.authorize("t", Scope::IncidentWrite)
            .await
            .expect("aceito antes da revogação");

        // Revoga na fonte persistente sem reiniciar o processo. Dentro do TTL o
        // cache ainda serve a ausência de revogação; passado o TTL, a próxima
        // requisição relê a fonte e nega.
        *revocation.revoked.lock().await = Some(151);
        clock.set(154);
        assert!(
            auth.authorize("t", Scope::IncidentWrite).await.is_ok(),
            "cache de 5 s ainda é fresco em 154"
        );
        clock.set(156);
        assert_eq!(
            auth.authorize("t", Scope::IncidentWrite).await,
            Err(AuthError::CredentialRevoked),
            "passado o TTL, a revogação é relida sem reinício"
        );
    }

    #[tokio::test]
    async fn explicit_invalidation_skips_the_ttl() {
        let registry = GrantRegistry::from_records([(
            "t".to_owned(),
            record(vec![Scope::IncidentWrite]),
        )]);
        let revocation = Arc::new(MemoryRevocation::default());
        let clock = Arc::new(StepClock::new(150));
        let auth = Authorization::with_parts(
            registry,
            clock,
            revocation.clone(),
            300,
            Arc::new(CapturedAudit::default()),
        );
        auth.authorize("t", Scope::IncidentWrite)
            .await
            .expect("aceito antes da revogação");
        *revocation.revoked.lock().await = Some(151);
        auth.invalidate_revocation("grant-1").await;
        assert_eq!(
            auth.authorize("t", Scope::IncidentWrite).await,
            Err(AuthError::CredentialRevoked),
            "invalidação explícita não espera o TTL"
        );
    }

    #[test]
    fn config_rejects_bad_windows_and_duplicates() {
        let base = r#"{"grants":[{"token":"t1","token_id":"g1","tenant_id":"a",
            "principal":"p","scopes":["incident:write"],"not_before":10,"expires_at":20,
            "authority_epoch":1,"operation":"open_sos_incident",
            "freshness":"f3_verified_online","revocation":"absent",
            "epoch_comparable":true,"action_enumerated":true,
            "retention_class_known":true}]}"#;
        assert_eq!(GrantRegistry::from_json(base).expect("config válida").len(), 1);

        let inverted = base.replace("\"not_before\":10,\"expires_at\":20", "\"not_before\":20,\"expires_at\":20");
        assert!(GrantRegistry::from_json(&inverted).is_err());

        let no_scope = base.replace("[\"incident:write\"]", "[]");
        assert!(GrantRegistry::from_json(&no_scope).is_err());

        let duplicate_scope = base.replace("[\"incident:write\"]", "[\"incident:write\",\"incident:write\"]");
        assert!(GrantRegistry::from_json(&duplicate_scope).is_err());

        let unknown = base.replace("\"tenant_id\":\"a\",", "\"tenant_id\":\"a\",\"tenant\":\"b\",");
        assert!(GrantRegistry::from_json(&unknown).is_err());

        // Dois grants: token repetido falha, token distinto com token_id
        // repetido também falha. Construído com serde_json para não depender de
        // manipulação de string.
        let mut second = serde_json::json!({
            "token": "t2",
            "token_id": "g2",
            "tenant_id": "a",
            "principal": "p",
            "scopes": ["incident:write"],
            "not_before": 10,
            "expires_at": 20,
            "authority_epoch": 1,
            "operation": "open_sos_incident",
            "freshness": "f3_verified_online",
            "revocation": "absent",
            "epoch_comparable": true,
            "action_enumerated": true,
            "retention_class_known": true
        });
        let first = serde_json::from_str::<serde_json::Value>(base)
            .expect("fixture base")
            ["grants"][0]
            .clone();
        let registry_with = |a: &serde_json::Value, b: &serde_json::Value| {
            serde_json::json!({ "grants": [a.clone(), b.clone()] }).to_string()
        };
        assert!(GrantRegistry::from_json(&registry_with(&first, &second)).is_ok());

        second["token"] = serde_json::json!("t1");
        assert!(
            GrantRegistry::from_json(&registry_with(&first, &second)).is_err(),
            "token duplicado precisa falhar"
        );

        second["token"] = serde_json::json!("t2");
        second["token_id"] = serde_json::json!("g1");
        assert!(
            GrantRegistry::from_json(&registry_with(&first, &second)).is_err(),
            "token_id duplicado precisa falhar"
        );
    }

    #[test]
    fn debug_and_errors_never_contain_the_token() {
        let secret = "segredo-que-nao-pode-vazar";
        let registry = GrantRegistry::from_records([(
            secret.to_owned(),
            record(vec![Scope::IncidentWrite]),
        )]);
        let rendered = format!("{registry:?}");
        assert!(!rendered.contains(secret), "Debug não pode expor o token");
        let error = AuthError::CredentialExpired;
        assert!(!error.to_string().contains(secret));
        assert!(!format!("{error:?}").contains(secret));
    }

    #[test]
    fn cache_ttl_zero_disables_reuse() {
        assert_eq!(cache_ttl_seconds(Duration::ZERO), 0);
        assert_eq!(cache_ttl_seconds(Duration::from_secs(5)), 5);
    }
}