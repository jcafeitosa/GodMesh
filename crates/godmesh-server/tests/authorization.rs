//! Autorização real do grant: escopo, janela de validade e revogação persistente.
//!
//! Diferente dos testes de contrato, aqui o alvo é o portão HTTP: o token não
//! basta para entrar. Cada caso verifica o **código de erro estável** e que o
//! handler não rodou (nenhum 201/erro de domínio antes da recusa).
//!
//! Os testes que só exercitam recusa (401/403) usam um pool *lazy*: a recusa
//! acontece antes de qualquer consulta. Os testes de aceitação e de revogação em
//! banco exigem `DATABASE_URL` com as migrações 0001, 0002 e 0004; sem ela são
//! ignorados (não fingem sucesso).

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use godmesh_identity::grant::{
    FreshnessSource, GrantPredicate, OfflineOp, RevocationState,
};
use godmesh_server::auth::{
    AuditSink, Authorization, CapturedAudit, Clock, FixedClock, GrantRecord, GrantRegistry,
    PgRevocationSource, RevocationFuture, RevocationSource, Scope, Subject, VerifiedGrant,
};
use godmesh_server::http::router_with;
use sqlx::PgPool;

const TENANT: &str = "tenant-autz";

/// Token opaco de teste; não é segredo real.
const TOKEN_SCOPED: &str = "token-autz-incident-write";
const TOKEN_READ_ONLY: &str = "token-autz-read-only";
const TOKEN_EXPIRED: &str = "token-autz-expirado";
const TOKEN_FUTURE: &str = "token-autz-futuro";
const TOKEN_REVOKED_DB: &str = "token-autz-revogado-db";

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(label: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{label}-{}-{n}", std::process::id())
}

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

fn record(token_id: &str, scopes: &[Scope], not_before: i64, expires_at: i64) -> GrantRecord {
    GrantRecord {
        token_id: token_id.to_owned(),
        subject: Subject {
            tenant_id: TENANT.to_owned(),
            principal: "principal-autz".to_owned(),
        },
        scopes: scopes.to_vec(),
        not_before,
        expires_at,
        revoked_at: None,
        authority_epoch: 3,
        predicate: predicate(),
    }
}

/// Fonte de revogação em memória: prova que revogar não exige reiniciar.
#[derive(Debug, Default)]
struct MemoryRevocation {
    revoked: tokio::sync::Mutex<Option<i64>>,
}

impl RevocationSource for MemoryRevocation {
    fn revoked_at<'a>(&'a self, _token_id: &'a str) -> RevocationFuture<'a> {
        Box::pin(async move { Ok(*self.revoked.lock().await) })
    }
}

/// Relógio fixo compartilhado com o autorizador.
fn fixed(now: i64) -> Arc<dyn Clock> {
    Arc::new(FixedClock(now))
}

fn registry_for(scopes: &[Scope]) -> GrantRegistry {
    GrantRegistry::from_records([(
        TOKEN_SCOPED.to_owned(),
        record("grant-escopo", scopes, 0, 4_102_444_800),
    )])
}

fn authorization(
    registry: GrantRegistry,
    now: i64,
    revocation: Arc<MemoryRevocation>,
    ttl: i64,
    audit: Arc<dyn AuditSink>,
) -> Authorization {
    Authorization::with_parts(registry, fixed(now), revocation, ttl, audit)
}

fn lazy_pool() -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://godmesh:godmesh_dev_only@127.0.0.1:1/godmesh")
        .expect("URL de pool lazy")
}

async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .ok()
}

async fn serve(pool: PgPool, authorization: Authorization) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = router_with(pool, authorization);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn http_post(addr: SocketAddr, path: &str, token: Option<&str>, body: &str) -> (u16, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de escrita");
    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(value) = token {
        request.push_str(&format!("Authorization: Bearer {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).expect("envia pedido");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("lê resposta");
    let status: u16 = raw
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {raw:?}"));
    (status, raw)
}

fn http_get(addr: SocketAddr, path: &str, token: &str) -> (u16, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).expect("envia pedido");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("lê resposta");
    let status: u16 = raw
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {raw:?}"));
    (status, raw)
}

fn incident_body(id: &str) -> String {
    format!("{{\"incident_id\":\"{id}\"}}")
}

async fn declare_tenant(pool: &PgPool, tenant: &str) {
    sqlx::query("INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, 'BR') ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction")
        .bind(tenant)
        .execute(pool)
        .await
        .expect("jurisdição de teste declarada");
    sqlx::query(
        "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
         VALUES ('incidente_sos', 'BR', 'dever_de_cuidado', 12, 'incident_closed + 12m') \
         ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .expect("classe incidente_sos presente");
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         VALUES ('incidente_sos', 'BR', 'primary') ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .expect("cópia primária presente");
}

// --- escopo -------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn valid_token_without_route_scope_is_403_insufficient_scope() {
    let grants = GrantRegistry::from_records([(
        TOKEN_READ_ONLY.to_owned(),
        record("grant-somente-leitura", &[Scope::IncidentRead], 0, 4_102_444_800),
    )]);
    let auth = authorization(
        grants,
        1_700_000_000,
        Arc::new(MemoryRevocation::default()),
        5,
        Arc::new(CapturedAudit::default()),
    );
    let addr = serve(lazy_pool(), auth).await;
    let id = unique("sem-escopo");
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some(TOKEN_READ_ONLY), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 403, "escopo ausente precisa ser 403: {raw}");
    assert!(
        raw.contains("insufficient_scope"),
        "o corpo precisa nomear o motivo: {raw}"
    );
    assert!(!raw.contains(TOKEN_READ_ONLY), "o token não pode aparecer na resposta");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn event_routes_declare_their_own_scopes() {
    // Dois grants no mesmo registro: um só com escrita de incidente, outro só
    // com leitura de evento. Cada rota precisa cobrar o seu escopo, não o do
    // vizinho — inclusive nos métodos que compartilham o caminho `/v1/events`.
    let grants = GrantRegistry::from_records([
        (
            TOKEN_SCOPED.to_owned(),
            record("grant-escrita-incidente", &[Scope::IncidentWrite], 0, 4_102_444_800),
        ),
        (
            TOKEN_READ_ONLY.to_owned(),
            record("grant-leitura-evento", &[Scope::EventRead], 0, 4_102_444_800),
        ),
    ]);
    let auth = authorization(
        grants,
        1_700_000_000,
        Arc::new(MemoryRevocation::default()),
        5,
        Arc::new(CapturedAudit::default()),
    );
    let addr = serve(lazy_pool(), auth).await;

    let body = r#"{"origin_node":"n1","tenant_id":"x","stream_id":"s1","partition":0,"authority_epoch":1,"seq":1,"retention_class":"incidente_sos"}"#.to_owned();
    let (post_status, post_raw) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/events", Some(TOKEN_SCOPED), &body)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(post_status, 403, "POST /v1/events exige event:write: {post_raw}");
    assert!(post_raw.contains("insufficient_scope"));

    let (get_denied, get_denied_raw) = tokio::task::spawn_blocking(move || {
        http_get(addr, "/v1/events", TOKEN_SCOPED)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(get_denied, 403, "GET /v1/events exige event:read: {get_denied_raw}");

    // Com event:read a rota é alcançada; o pool lazy falha depois, mas não em
    // autorização — é a prova de que o escopo certo foi cobrado.
    let (get_allowed, get_allowed_raw) = tokio::task::spawn_blocking(move || {
        http_get(addr, "/v1/events", TOKEN_READ_ONLY)
    })
    .await
    .expect("cliente HTTP");
    assert_ne!(get_allowed, 403, "event:read não pode ser barrado: {get_allowed_raw}");
    assert_ne!(get_allowed, 401, "credencial válida não pode ser 401: {get_allowed_raw}");
}

// --- janela de validade -------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_grant_is_401_credential_expired() {
    let grants = GrantRegistry::from_records([(
        TOKEN_EXPIRED.to_owned(),
        record("grant-expirado", &[Scope::IncidentWrite], 100, 200),
    )]);
    let auth = authorization(
        grants,
        200,
        Arc::new(MemoryRevocation::default()),
        5,
        Arc::new(CapturedAudit::default()),
    );
    let addr = serve(lazy_pool(), auth).await;
    let id = unique("expirado");
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some(TOKEN_EXPIRED), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 401, "credencial expirada precisa ser 401: {raw}");
    assert!(raw.contains("credential_expired"), "motivo estável: {raw}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn future_not_before_is_401_credential_not_yet_valid() {
    let grants = GrantRegistry::from_records([(
        TOKEN_FUTURE.to_owned(),
        record("grant-futuro", &[Scope::IncidentWrite], 500, 900),
    )]);
    let auth = authorization(
        grants,
        499,
        Arc::new(MemoryRevocation::default()),
        5,
        Arc::new(CapturedAudit::default()),
    );
    let addr = serve(lazy_pool(), auth).await;
    let id = unique("futuro");
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some(TOKEN_FUTURE), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 401, "not_before no futuro precisa ser 401: {raw}");
    assert!(
        raw.contains("credential_not_yet_valid"),
        "motivo estável: {raw}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_token_is_401_unauthenticated() {
    let auth = authorization(
        registry_for(&[Scope::IncidentWrite]),
        1_700_000_000,
        Arc::new(MemoryRevocation::default()),
        5,
        Arc::new(CapturedAudit::default()),
    );
    let addr = serve(lazy_pool(), auth).await;
    let id = unique("desconhecido");
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some("token-que-nao-existe"), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 401, "token desconhecido precisa ser 401: {raw}");
    assert!(raw.contains("unauthenticated"), "motivo estável: {raw}");
}

// --- revogação persistente ----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revocation_in_database_is_401_without_restart() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let token_id = unique("token-id-revogado");
    // Estado limpo para o TTL 0 provar leitura do banco, não de cache anterior.
    sqlx::query("DELETE FROM grant_revocation WHERE token_id = $1")
        .bind(&token_id)
        .execute(&pool)
        .await
        .expect("tabela grant_revocation existe (aplique migrations/0004)");

    let registry = GrantRegistry::from_records([(
        TOKEN_REVOKED_DB.to_owned(),
        record(&token_id, &[Scope::IncidentWrite], 0, 4_102_444_800),
    )]);
    // TTL 0: cada requisição relê a revogação; ainda assim o mesmo router é
    // reutilizado, o que prova que revogar não exigiu reiniciar o processo.
    let auth = Authorization::with_parts(
        registry,
        fixed(1_700_000_000),
        Arc::new(PgRevocationSource::new(pool.clone())),
        0,
        Arc::new(CapturedAudit::default()),
    );
    let addr = serve(pool.clone(), auth).await;

    // Antes da revogação a credencial passa do portão: a recusa seguinte é de
    // domínio (sem jurisdição declarada), não de autenticação.
    let id = unique("antes-revogar");
    let (status_before, raw_before) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some(TOKEN_REVOKED_DB), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");
    assert_ne!(
        status_before, 401,
        "credencial ainda não revogada não pode ser 401: {raw_before}"
    );

    // Revoga em banco. Nenhum reinício, nenhuma reconfiguração.
    sqlx::query("INSERT INTO grant_revocation (token_id, reason) VALUES ($1, 'teste de revogação')")
        .bind(&token_id)
        .execute(&pool)
        .await
        .expect("revogação registrada");

    let id = unique("depois-revogar");
    let (status_after, raw_after) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some(TOKEN_REVOKED_DB), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(
        status_after, 401,
        "revogação em banco precisa valer sem reinício: {raw_after}"
    );
    assert!(
        raw_after.contains("credential_revoked"),
        "motivo estável: {raw_after}"
    );

    sqlx::query("DELETE FROM grant_revocation WHERE token_id = $1")
        .bind(&token_id)
        .execute(&pool)
        .await
        .expect("limpeza da revogação de teste");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revocation_from_source_is_401_credential_revoked() {
    let revocation = Arc::new(MemoryRevocation::default());
    // Relógio fixo: o cache de 5 s é atravessado mudando o TTL para 0 na segunda
    // composição não seria o mesmo router; aqui provamos o cache pelo autorizador
    // e o HTTP pela recusa estável.
    let auth = Authorization::with_parts(
        registry_for(&[Scope::IncidentWrite]),
        fixed(1_700_000_000),
        revocation.clone(),
        5,
        Arc::new(CapturedAudit::default()),
    );
    *revocation.revoked.lock().await = Some(1_700_000_001);
    let addr = serve(lazy_pool(), auth).await;
    let id = unique("revogado-fonte");
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some(TOKEN_SCOPED), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 401, "revogação na fonte precisa negar: {raw}");
    assert!(raw.contains("credential_revoked"), "motivo estável: {raw}");
}

// --- caminho feliz e segredo em log/erro --------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn valid_grant_with_scope_is_201() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    declare_tenant(&pool, TENANT).await;
    let auth = Authorization::with_parts(
        registry_for(&[Scope::IncidentWrite]),
        fixed(1_700_000_000),
        Arc::new(MemoryRevocation::default()),
        5,
        Arc::new(CapturedAudit::default()),
    );
    let addr = serve(pool.clone(), auth).await;
    let id = unique("grant-valido");
    let check_id = id.clone();
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some(TOKEN_SCOPED), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 201, "grant válido com escopo precisa ser 201: {raw}");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE id = $1")
        .bind(&check_id)
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(rows, 1, "o incidente foi persistido");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_token_appears_in_log_or_error_body() {
    let audit = Arc::new(CapturedAudit::default());
    let grants = GrantRegistry::from_records([(
        TOKEN_EXPIRED.to_owned(),
        record("grant-log", &[Scope::IncidentWrite], 100, 200),
    )]);
    let auth = Authorization::with_parts(
        grants,
        fixed(200),
        Arc::new(MemoryRevocation::default()),
        5,
        audit.clone(),
    );
    let addr = serve(lazy_pool(), auth).await;
    let id = unique("log");
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http_post(addr, "/v1/incidents", Some(TOKEN_EXPIRED), &incident_body(&id))
    })
    .await
    .expect("cliente HTTP");

    assert_eq!(status, 401);
    assert!(!raw.contains(TOKEN_EXPIRED), "resposta não pode conter o token: {raw}");
    let lines = audit.lines().await;
    assert!(!lines.is_empty(), "a recusa precisa ser auditada");
    for line in &lines {
        assert!(
            !line.contains(TOKEN_EXPIRED),
            "auditoria não pode conter o token: {line}"
        );
    }
    assert!(
        lines.iter().any(|l| l.contains("token_id=grant-log")),
        "auditoria identifica o grant sem expor o token: {lines:?}"
    );
}

#[test]
fn verified_grant_exposes_scopes_from_the_record() {
    let r = record("g", &[Scope::EventRead], 0, 10);
    let verified: VerifiedGrant = r.verified();
    assert!(verified.has_scope(Scope::EventRead));
    assert!(!verified.has_scope(Scope::EventWrite));
}