//! Ciclo administrativo de grants por API: emitir → usar → revogar, sem reinício.
//!
//! O alvo é o portão HTTP real, com o router de produção
//! (`godmesh_server::http::router_with`) e o banco de verdade. Cada passo confere
//! o **efeito** (linha no banco, incidente persistido) e não só o status.
//!
//! Banco: descartável (`godmesh_admin_<pid>`), criado do zero e migrado pelo
//! *runner oficial* (`tests/common/mod.rs`). Sem `DATABASE_URL` os testes são
//! pulados com aviso; com URL declarada e banco inacessível eles **falham**.
//!
//! Execução (local-first, ADR-070 — sem contêiner):
//!
//! ```text
//! DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
//!   cargo test -p godmesh-server --test admin_grants -- --nocapture
//! ```

mod common;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{
    token_hash, Authorization, CapturedAudit, GrantRecord, GrantRegistry, PgIssuedGrantSource,
    PgRevocationSource, Scope, Subject, SystemClock,
};
use godmesh_server::http::router_with;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};

/// Tenant usado em todos os caminhos felizes.
const TENANT: &str = "tenant-grant-br";
/// Token administrativo provisionado fora da API (é o bootstrap permitido).
const TOKEN_ADMIN: &str = "token-admin-grants-teste";
/// Token sem `admin:grants`: pode ler incidente, não pode administrar grant.
const TOKEN_LEITURA: &str = "token-leitura-sem-admin";

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(label: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{label}-{}-{n}", std::process::id())
}

// --- ambiente ---------------------------------------------------------------

struct Env {
    url: String,
}

static ENV: tokio::sync::OnceCell<Env> = tokio::sync::OnceCell::const_new();

fn db_name() -> String {
    format!("godmesh_admin_{}", std::process::id())
}

fn with_database(url: &str, db: &str) -> String {
    let (sem_query, query) = match url.split_once('?') {
        Some((a, b)) => (a, Some(b)),
        None => (url, None),
    };
    let corte = sem_query
        .rfind('/')
        .expect("DATABASE_URL precisa terminar em /banco");
    let raiz = &sem_query[..corte];
    match query {
        Some(q) => format!("{raiz}/{db}?{q}"),
        None => format!("{raiz}/{db}"),
    }
}

async fn setup() -> Env {
    let base = common::database_url().expect("URL de banco presente (verificado antes)");
    let db = db_name();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_database(&base, "postgres"))
        .await
        .unwrap_or_else(|e| panic!("Postgres inacessível para administração ({base}): {e}"));
    sqlx::query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {db} WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .expect("drop do banco descartável");
    sqlx::query(AssertSqlSafe(format!("CREATE DATABASE {db}")))
        .execute(&admin)
        .await
        .expect("create do banco descartável");
    admin.close().await;

    let url = with_database(&base, &db);
    let pool = common::pool_para(&url, 4).await;
    sqlx::query(
        "INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, 'BR') \
         ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction",
    )
    .bind(TENANT)
    .execute(&pool)
    .await
    .expect("jurisdição BR declarada para o tenant de teste");
    sqlx::query(
        "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
         SELECT 'incidente_sos', 'BR', 'dever_de_cuidado', 12, 'incident_closed + 12m' \
          WHERE NOT EXISTS (SELECT 1 FROM retention_class \
                             WHERE class = 'incidente_sos' AND jurisdiction = 'BR')",
    )
    .execute(&pool)
    .await
    .expect("classe incidente_sos presente");
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         SELECT 'incidente_sos', 'BR', 'primary' \
          WHERE NOT EXISTS (SELECT 1 FROM retention_copy \
                             WHERE class = 'incidente_sos' AND jurisdiction = 'BR' \
                               AND copy_scope = 'primary')",
    )
    .execute(&pool)
    .await
    .expect("cópia primária presente");
    pool.close().await;

    eprintln!("admin-grants: banco descartável pronto em {url}");
    Env { url }
}

async fn env() -> Option<&'static Env> {
    if common::database_url().is_none() {
        eprintln!(
            "SKIP: sem GODMESH_TEST_DATABASE_URL/DATABASE_URL; a suíte de grants depende de Postgres"
        );
        return None;
    }
    Some(ENV.get_or_init(setup).await)
}

async fn pool(env: &Env) -> PgPool {
    common::pool_para(&env.url, 4).await
}

// --- composição do router ---------------------------------------------------

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

fn record(token_id: &str, scopes: Vec<Scope>) -> GrantRecord {
    GrantRecord {
        token_id: token_id.to_owned(),
        subject: Subject {
            tenant_id: TENANT.to_owned(),
            principal: "operador-teste".to_owned(),
        },
        scopes,
        not_before: 0,
        expires_at: i64::MAX,
        revoked_at: None,
        authority_epoch: 7,
        predicate: predicate(),
    }
}

fn registry() -> GrantRegistry {
    GrantRegistry::from_records([
        (TOKEN_ADMIN.to_owned(), record("g-admin", vec![Scope::AdminGrants])),
        (
            TOKEN_LEITURA.to_owned(),
            record("g-leitura", vec![Scope::IncidentRead]),
        ),
    ])
}

/// Autorização de produção, com duas diferenças deliberadas:
///  * TTL de revogação longo (300 s), para provar que revogar vale **agora** por
///    invalidação explícita, não por expiração de cache;
///  * auditoria capturada em memória, para provar que o token não vai para log.
fn authorization(
    pool: &PgPool,
    audit: Arc<CapturedAudit>,
    cache_ttl_seconds: i64,
) -> Authorization {
    Authorization::with_parts(
        registry(),
        Arc::new(SystemClock),
        Arc::new(PgRevocationSource::new(pool.clone())),
        cache_ttl_seconds,
        audit,
    )
    .with_issued_source(Arc::new(PgIssuedGrantSource::new(pool.clone())))
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

// --- cliente HTTP/1.1 sem dependência nova -----------------------------------

struct Response {
    status: u16,
    body: String,
}

impl Response {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| {
            panic!(
                "corpo não é JSON ({e}): {:?} (status {})",
                self.body, self.status
            )
        })
    }
}

fn call(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> Response {
    let mut stream = TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de escrita");

    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\n");
    if let Some(value) = token {
        request.push_str(&format!("Authorization: Bearer {value}\r\n"));
    }
    if let Some(body) = body {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("Connection: close\r\n\r\n");
    if let Some(body) = body {
        request.push_str(body);
    }

    stream.write_all(request.as_bytes()).expect("envia pedido");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("lê resposta");
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("resposta HTTP sem cabeçalho: {raw:?}"));
    let status: u16 = head
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {raw:?}"));
    Response {
        status,
        body: body.to_owned(),
    }
}

fn get(addr: SocketAddr, path: &str, token: &str) -> Response {
    call(addr, "GET", path, Some(token), None)
}

fn post(addr: SocketAddr, path: &str, token: &str, body: &str) -> Response {
    call(addr, "POST", path, Some(token), Some(body))
}

fn issue_body(tenant: &str, scopes: &[&str], ttl_seconds: i64) -> String {
    json!({
        "tenant_id": tenant,
        "principal": "operador-emissor",
        "scopes": scopes,
        "ttl_seconds": ttl_seconds,
    })
    .to_string()
}

fn revoke_body(reason: &str) -> String {
    json!({ "reason": reason }).to_string()
}

fn incident_body(id: &str) -> String {
    json!({ "incident_id": id }).to_string()
}

// --- emissão pelo HTTP -------------------------------------------------------

/// Emite via rota administrativa e devolve a resposta já validada como 201.
fn emit(addr: SocketAddr, body: &str) -> Value {
    let res = post(addr, "/v1/admin/grants", TOKEN_ADMIN, body);
    assert_eq!(res.status, 201, "emissão precisa ser 201: {}", res.body);
    res.json()
}

// --- 1) ciclo completo -------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn emite_usa_revoga_e_token_morre_sem_reiniciar() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    // TTL de cache longo: se a revogação valesse só por expiração, o teste
    // falharia; ela vale por invalidação explícita no mesmo processo.
    let addr = serve(
        pool.clone(),
        authorization(&pool, Arc::new(CapturedAudit::default()), 300),
    )
    .await;

    let issued = emit(
        addr,
        &issue_body(TENANT, &["incident:write"], 3_600),
    );
    let token = issued["token"]
        .as_str()
        .expect("resposta devolve o token uma vez")
        .to_owned();
    let token_id = issued["token_id"].as_str().expect("token_id").to_owned();
    assert!(token.starts_with("gm_"), "token opaco: {token}");
    assert_eq!(issued["tenant_id"], TENANT);
    assert_eq!(issued["principal"], "operador-emissor");
    assert_eq!(issued["scopes"], json!(["incident:write"]));
    assert_eq!(issued["authority_epoch"], 1);
    let expires_at = issued["expires_at"].as_i64().expect("expires_at");
    assert!(expires_at > issued["not_before"].as_i64().expect("not_before"));

    // Só o hash vai para o banco; o token em claro não tem coluna.
    let stored: String = sqlx::query_scalar("SELECT token_hash FROM grant_issued WHERE token_id = $1")
        .bind(&token_id)
        .fetch_one(&pool)
        .await
        .expect("grant emitido persistido");
    assert_eq!(stored, token_hash(&token), "o banco guarda o sha256 do token");
    assert_ne!(stored, token, "o token em claro não é persistido");
    assert_eq!(stored.len(), 64);

    // O token emitido vale nas rotas de domínio, sem reiniciar.
    let incident = unique("emitido");
    let res = post(addr, "/v1/incidents", &token, &incident_body(&incident));
    assert_eq!(res.status, 201, "token emitido precisa ser aceito: {}", res.body);
    let res = post(addr, "/v1/incidents", &token, &incident_body(&incident));
    assert_eq!(res.status, 200, "replay idempotente: {}", res.body);

    // Metadados: sem token e sem hash.
    let meta = get(addr, &format!("/v1/admin/grants/{token_id}"), TOKEN_ADMIN);
    assert_eq!(meta.status, 200, "metadados: {}", meta.body);
    assert!(
        !meta.body.contains(&token),
        "metadados não podem repetir o token: {}",
        meta.body
    );
    assert!(
        !meta.body.contains(&token_hash(&token)),
        "metadados não podem expor o hash: {}",
        meta.body
    );
    let meta_json = meta.json();
    assert!(meta_json.get("token").is_none());
    assert!(meta_json.get("token_hash").is_none());
    assert_eq!(meta_json["revoked"], false);

    // Revoga com motivo obrigatório.
    let res = post(
        addr,
        &format!("/v1/admin/grants/{token_id}/revoke"),
        TOKEN_ADMIN,
        &revoke_body("rotacao de credencial"),
    );
    assert_eq!(res.status, 200, "revogação: {}", res.body);
    let revoked_at = res.json()["revoked_at"].as_i64().expect("revoked_at");
    assert!(revoked_at > 0);

    // O MESMO token, no MESMO processo, agora é recusado.
    let after = unique("revogado");
    let res = post(addr, "/v1/incidents", &token, &incident_body(&after));
    assert_eq!(
        res.status, 401,
        "token revogado precisa ser 401 sem reinício: {}",
        res.body
    );
    assert_eq!(res.json()["error"], "credential_revoked");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE id = $1")
        .bind(&after)
        .fetch_one(&pool)
        .await
        .expect("contagem de incidente");
    assert_eq!(rows, 0, "requisição recusada não pode gravar incidente");

    // Metadados refletem a revogação e o motivo.
    let meta = get(addr, &format!("/v1/admin/grants/{token_id}"), TOKEN_ADMIN).json();
    assert_eq!(meta["revoked"], true);
    assert_eq!(meta["revocation_reason"], "rotacao de credencial");
    assert_eq!(meta["revoked_at"].as_i64(), Some(revoked_at));

    // Revogar de novo é idempotente: o instante original não é reescrito.
    let again = post(
        addr,
        &format!("/v1/admin/grants/{token_id}/revoke"),
        TOKEN_ADMIN,
        &revoke_body("segunda chamada"),
    );
    assert_eq!(again.status, 200, "revogação repetida: {}", again.body);
    assert_eq!(again.json()["revoked_at"].as_i64(), Some(revoked_at));
}

// --- 2) escopo de leitura não escreve ---------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn grant_so_de_leitura_nao_escreve_incidente() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(
        pool.clone(),
        authorization(&pool, Arc::new(CapturedAudit::default()), 5),
    )
    .await;

    let issued = emit(addr, &issue_body(TENANT, &["incident:read"], 3_600));
    let token = issued["token"].as_str().expect("token").to_owned();

    let id = unique("leitura");
    let res = post(addr, "/v1/incidents", &token, &incident_body(&id));
    assert_eq!(
        res.status, 403,
        "escopo de leitura não escreve: {}",
        res.body
    );
    assert_eq!(res.json()["error"], "insufficient_scope");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE id = $1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .expect("contagem");
    assert_eq!(rows, 0, "403 não pode gravar");

    // A leitura funciona com o mesmo token (404 de incidente inexistente, não
    // 403): prova que o escopo concedido é de leitura, não ausência de escopo.
    let res = get(addr, &format!("/v1/incidents/{id}"), &token);
    assert_eq!(res.status, 404, "leitura permitida: {}", res.body);
    assert_eq!(res.json()["error"], "incident_not_found");
}

// --- 3) expiração ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn token_expirado_e_401_credential_expired() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(
        pool.clone(),
        authorization(&pool, Arc::new(CapturedAudit::default()), 5),
    )
    .await;

    // Estado criado em SQL para não esperar o relógio: o token é real, a janela
    // já passou. O hash é o mesmo que o servidor calcularia.
    let token = format!("gm_{}", unique("expirado"));
    let token_id = unique("gtr-expirado");
    sqlx::query(
        "INSERT INTO grant_issued \
             (token_id, tenant_id, principal, token_hash, scopes, not_before, expires_at, authority_epoch) \
         VALUES ($1, $2, 'operador-emissor', $3, ARRAY['incident:write'], \
                 now() - interval '2 hours', now() - interval '1 hour', 1)",
    )
    .bind(&token_id)
    .bind(TENANT)
    .bind(token_hash(&token))
    .execute(&pool)
    .await
    .expect("grant expirado persistido");

    let res = post(
        addr,
        "/v1/incidents",
        &token,
        &incident_body(&unique("expirado-uso")),
    );
    assert_eq!(res.status, 401, "expirado: {}", res.body);
    assert_eq!(res.json()["error"], "credential_expired");

    // Metadados continuam consultáveis: expirar não apaga a história.
    let meta = get(addr, &format!("/v1/admin/grants/{token_id}"), TOKEN_ADMIN);
    assert_eq!(meta.status, 200);
    assert_eq!(meta.json()["revoked"], false);
}

// --- 4) segredo não vaza -----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn token_nao_aparece_em_resposta_posterior_nem_em_log() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let audit = Arc::new(CapturedAudit::default());
    let addr = serve(pool.clone(), authorization(&pool, audit.clone(), 5)).await;

    let issued = emit(addr, &issue_body(TENANT, &["incident:write"], 600));
    let token = issued["token"].as_str().expect("token").to_owned();
    let token_id = issued["token_id"].as_str().expect("token_id").to_owned();

    let incident = unique("segredo");
    let res = post(addr, "/v1/incidents", &token, &incident_body(&incident));
    assert_eq!(res.status, 201, "uso do token: {}", res.body);
    assert!(
        !res.body.contains(&token),
        "resposta de domínio não repete a credencial"
    );

    let meta = get(addr, &format!("/v1/admin/grants/{token_id}"), TOKEN_ADMIN);
    assert_eq!(meta.status, 200);
    assert!(
        !meta.body.contains(&token) && !meta.body.contains(&token_hash(&token)),
        "consulta administrativa não expõe token nem hash: {}",
        meta.body
    );
    post(
        addr,
        &format!("/v1/admin/grants/{token_id}/revoke"),
        TOKEN_ADMIN,
        &revoke_body("descarte"),
    );

    let lines = audit.lines().await;
    assert!(!lines.is_empty(), "operações precisam ser auditadas");
    for line in &lines {
        assert!(
            !line.contains(&token),
            "auditoria (log) não pode conter o token: {line}"
        );
        assert!(
            !line.contains(&token_hash(&token)),
            "auditoria (log) não pode conter o hash do token: {line}"
        );
    }
    assert!(
        lines.iter().any(|l| l.contains("event=grant_issued") && l.contains(&token_id)),
        "emissão auditada pelo token_id: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("event=grant_revoked") && l.contains(&token_id)),
        "revogação auditada pelo token_id: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("token_id=g-admin")),
        "a credencial administrativa que agiu é identificada: {lines:?}"
    );
}

// --- 5) autorização da própria rota -----------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rotas_administrativas_exigem_escopo_admin_grants() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(
        pool.clone(),
        authorization(&pool, Arc::new(CapturedAudit::default()), 5),
    )
    .await;

    let res = post(
        addr,
        "/v1/admin/grants",
        TOKEN_LEITURA,
        &issue_body(TENANT, &["incident:read"], 60),
    );
    assert_eq!(res.status, 403, "emissão exige admin:grants: {}", res.body);
    assert_eq!(res.json()["error"], "insufficient_scope");

    let res = get(addr, "/v1/admin/grants/g-qualquer", TOKEN_LEITURA);
    assert_eq!(res.status, 403, "consulta exige admin:grants: {}", res.body);

    // Sem credencial nenhuma, 401 antes de qualquer efeito.
    let res = call(
        addr,
        "POST",
        "/v1/admin/grants",
        None,
        Some(&issue_body(TENANT, &["incident:read"], 60)),
    );
    assert_eq!(res.status, 401, "sem credencial: {}", res.body);
    assert_eq!(res.json()["error"], "unauthenticated");
}

// --- 6) recusas de fronteira -------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn emissoes_invalidas_e_token_desconhecido_sao_recusados() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(
        pool.clone(),
        authorization(&pool, Arc::new(CapturedAudit::default()), 5),
    )
    .await;

    // Tenant exclusivo deste teste: os testes rodam em paralelo no mesmo banco,
    // então a contagem precisa isolar o que este caso poderia ter emitido.
    let tenant = unique("tenant-invalido");
    let emitidos = |pool: PgPool, tenant: String| async move {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM grant_issued WHERE tenant_id = $1",
        )
        .bind(tenant)
        .fetch_one(&pool)
        .await
        .expect("contagem de grants")
    };
    let antes = emitidos(pool.clone(), tenant.clone()).await;

    for (label, body) in [
        ("ttl zero", issue_body(&tenant, &["incident:read"], 0)),
        ("ttl negativo", issue_body(&tenant, &["incident:read"], -1)),
        (
            "ttl acima do teto",
            issue_body(&tenant, &["incident:read"], 30 * 24 * 3600 + 1),
        ),
        ("sem escopo", issue_body(&tenant, &[], 60)),
        (
            "escopo repetido",
            issue_body(&tenant, &["incident:read", "incident:read"], 60),
        ),
        (
            "escopo administrativo",
            issue_body(&tenant, &["admin:grants"], 60),
        ),
        (
            "escopo desconhecido",
            issue_body(&tenant, &["incident:delete"], 60),
        ),
    ] {
        let res = post(addr, "/v1/admin/grants", TOKEN_ADMIN, &body);
        assert_eq!(res.status, 422, "{label} precisa ser 422: {}", res.body);
    }

    let depois = emitidos(pool.clone(), tenant.clone()).await;
    assert_eq!(antes, 0, "tenant exclusivo começa sem grant");
    assert_eq!(depois, 0, "recusa de fronteira não emite grant");

    // token_id desconhecido: 404 nas duas rotas, nunca 200 vazio.
    let res = get(addr, "/v1/admin/grants/gtr-inexistente", TOKEN_ADMIN);
    assert_eq!(res.status, 404, "consulta de desconhecido: {}", res.body);
    assert_eq!(res.json()["error"], "grant_not_found");
    let res = post(
        addr,
        "/v1/admin/grants/gtr-inexistente/revoke",
        TOKEN_ADMIN,
        &revoke_body("motivo qualquer"),
    );
    assert_eq!(res.status, 404, "revogação de desconhecido: {}", res.body);
    assert_eq!(res.json()["error"], "grant_not_found");

    // Motivo de revogação é obrigatório: vazio ou só espaço é 422, e o grant
    // continua válido (emitir, tentar revogar sem motivo, usar).
    let issued = emit(addr, &issue_body(TENANT, &["incident:write"], 600));
    let token = issued["token"].as_str().expect("token").to_owned();
    let token_id = issued["token_id"].as_str().expect("token_id").to_owned();
    for motivo in ["", "   "] {
        let res = post(
            addr,
            &format!("/v1/admin/grants/{token_id}/revoke"),
            TOKEN_ADMIN,
            &revoke_body(motivo),
        );
        assert_eq!(res.status, 422, "motivo vazio precisa ser 422: {}", res.body);
    }
    let res = post(
        addr,
        "/v1/incidents",
        &token,
        &incident_body(&unique("ainda-valido")),
    );
    assert_eq!(
        res.status, 201,
        "revogação recusada não pode invalidar o grant: {}",
        res.body
    );
}