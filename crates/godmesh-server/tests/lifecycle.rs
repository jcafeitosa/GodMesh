<<<<<<< HEAD
//! Ciclo de vida do incidente no servidor — contrato HTTP ponta a ponta.
//!
//! Cobre o caminho completo feliz (`persisted_local` → `replicated_to_authority`
//! → `acknowledged_technical` → `acknowledged_human` → `closed`), transição fora
//! de ordem (409 com o estado atual), reconhecimento humano sem prova (422),
//! isolamento entre tenants (404, nunca 403), concorrência na mesma transição
//! (exatamente uma sucesso e uma 409) e a trilha `sos_ack` append-only com
//! `at_unix` real.
//!
//! Banco próprio (não o de dev compartilhado), criado do zero com as migrações:
//!   createdb godmesh_sc && \
//!   psql -d godmesh_sc -v ON_ERROR_STOP=1 -f migrations/0001_core.sql \
//!                                  -f migrations/0002_incident_tenancy.sql
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_sc \
//!     cargo test -p godmesh-server --test lifecycle
//!
//! Sem DATABASE_URL os testes são ignorados (não fingem sucesso).

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_identity::grant::{
    FreshnessSource, GrantPredicate, OfflineOp, RevocationState,
};
use godmesh_server::auth::{GrantRegistry, Subject, VerifiedGrant};
use serde_json::Value;
use sqlx::PgPool;

const TOKEN_A: &str = "grant-opaco-sc-tenant-a";
const TOKEN_B: &str = "grant-opaco-sc-tenant-b";
const TENANT_A: &str = "tenant-sc-a";
const TENANT_B: &str = "tenant-sc-b";

/// Serializa este binário: as asserções de contagem global só valem sem outro
/// teste inserindo no meio (o banco é dedicado, mas `cargo test` paraleliza).
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Sufixo único por execução: o banco pode ser reaproveitado entre rodadas.
static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_id(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("inc-sc-{tag}-{}-{n}-{nanos}", std::process::id())
}

fn allow_grant(tenant_id: &str) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant_id.to_owned(),
            principal: "principal-sc".to_owned(),
        },
        predicate: GrantPredicate {
            operation: OfflineOp::OpenSosIncident,
            epoch_comparable: true,
            revocation: RevocationState::Absent,
            freshness: FreshnessSource::F3VerifiedOnline,
            retention_class_known: true,
            action_enumerated: true,
        },
    }
}

fn registry() -> GrantRegistry {
    GrantRegistry::from_entries(vec![
        (TOKEN_A.to_owned(), allow_grant(TENANT_A)),
        (TOKEN_B.to_owned(), allow_grant(TENANT_B)),
    ])
}

async fn pool() -> Option<PgPool> {
    let url = std::env::var("DATABASE_URL").ok()?;
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .ok()
}

/// Declara jurisdição e política do tenant de teste (a API não cria nenhuma).
async fn declare_tenant(pool: &PgPool, tenant: &str, jurisdiction: &str) {
    sqlx::query("INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, $2) ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction")
        .bind(tenant)
        .bind(jurisdiction)
        .execute(pool)
        .await
        .expect("jurisdição de teste declarada");
    sqlx::query(
        "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
         VALUES ('incidente_sos', $1, 'dever_de_cuidado', 12, 'incident_closed + 12m') \
         ON CONFLICT DO NOTHING",
    )
    .bind(jurisdiction)
    .execute(pool)
    .await
    .expect("classe incidente_sos presente");
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         VALUES ('incidente_sos', $1, 'primary') ON CONFLICT DO NOTHING",
    )
    .bind(jurisdiction)
    .execute(pool)
    .await
    .expect("cópia primária presente");
}

async fn serve(pool: PgPool) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = godmesh_server::http::router(pool, registry());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

/// Cliente HTTP/1.1 cru: status e corpo passam pelo fio, sem intermediário que
/// pudesse normalizar o que o servidor realmente devolve.
fn http(
    addr: SocketAddr,
    method: &str,
    path: &str,
    authorization: Option<&str>,
    body: Option<&str>,
) -> (u16, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(15)))
        .expect("timeout de escrita");
    let body = body.unwrap_or("");
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(value) = authorization {
        request.push_str(&format!("Authorization: {value}\r\n"));
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
=======
//! Ciclo de vida local do servidor: migrações, `/healthz` e shutdown.
//!
//! Banco próprio: `godmesh_sa` (nome configurável por `GODMESH_SA_TEST_DB`),
//! **criado pelo próprio binário** e migrado pelo binário (`--migrate-only`).
//! O banco de desenvolvimento (`godmesh`) nunca é tocado aqui.
//!
//! Conexão base: `DATABASE_URL` (ex.:
//! `postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh`). Só a parte do
//! banco é substituída. Sem `DATABASE_URL` os testes são ignorados — não fingem
//! sucesso.
//!
//! Como o banco derrubado é simulado (`/healthz` 503): `ALTER DATABASE godmesh_sa
//! ALLOW_CONNECTIONS false` + `pg_terminate_backend` nas sessões abertas naquele
//! banco, pelo pool de administração. Não derruba contêiner nem afeta outro
//! banco, e é reversível — a recuperação (`200` de novo) também é provada.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use godmesh_server::auth::GrantRegistry;
use godmesh_server::{http, lifecycle, migrate};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Nome do banco dos testes desta fatia.
const SA_DEFAULT: &str = "godmesh_sa";

/// Serializa os testes que mexem no banco `godmesh_sa`. `cargo test` roda os
/// testes de um binário em paralelo; sem o cadeado, um derrubaria a conexão do
/// outro no meio da prova.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn sole_lock() -> tokio::sync::MutexGuard<'static, ()> {
    DB_LOCK.lock().await
}

fn base_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn sa_db() -> String {
    std::env::var("GODMESH_SA_TEST_DB").unwrap_or_else(|_| SA_DEFAULT.to_owned())
}

/// Troca só o nome do banco na URL (preserva credenciais, host, porta e query).
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

fn sa_url(base: &str) -> String {
    with_database(base, &sa_db())
}

fn binario() -> &'static str {
    env!("CARGO_BIN_EXE_godmesh-server")
}

/// Diretório de migrações do repositório, resolvido pelo próprio manifesto.
fn migrations_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    dir.canonicalize()
        .expect("diretório migrations do repositório")
}

/// Nomes `*.sql` do diretório, em ordem — a mesma ordem do runner.
fn nomes_de_migracao(dir: &Path) -> Vec<String> {
    let mut nomes: Vec<String> = std::fs::read_dir(dir)
        .expect("lê migrations")
        .filter_map(|e| {
            let path = e.ok()?.path();
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("sql") {
                return None;
            }
            path.file_name()?.to_str().map(str::to_owned)
        })
        .collect();
    nomes.sort();
    nomes
}

async fn admin_pool(base: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_database(base, "postgres"))
        .await
        .expect("conexão de administração ao Postgres")
}

// `AssertSqlSafe` nos DDL de banco de teste: o nome do banco vem de
// `GODMESH_SA_TEST_DB`/constante deste arquivo, nunca de rede, e identificador
// não pode ser parâmetro de bind em `CREATE`/`DROP`/`ALTER DATABASE`.

/// Recria `godmesh_sa` do zero. `FORCE` derruba sessões remanescentes do teste
/// anterior — é banco descartável, criado por este binário de teste.
async fn recreate_sa(admin: &PgPool) {
    let db = sa_db();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {db} WITH (FORCE)"
    )))
    .execute(admin)
    .await
    .expect("drop do banco de teste");
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {db}")))
        .execute(admin)
        .await
        .expect("create do banco de teste");
}

/// Cria `godmesh_sa` se ainda não existir.
async fn ensure_sa(admin: &PgPool) {
    let db = sa_db();
    let existe: Option<String> =
        sqlx::query_scalar("SELECT datname FROM pg_database WHERE datname = $1")
            .bind(&db)
            .fetch_optional(admin)
            .await
            .expect("consulta pg_database");
    if existe.is_none() {
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {db}")))
            .execute(admin)
            .await
            .expect("create do banco de teste");
    }
}

fn run_migrate_only(url: &str, dir: &Path) -> Output {
    Command::new(binario())
        .arg("--migrate-only")
        .env("DATABASE_URL", url)
        .env("GODMESH_MIGRATIONS_DIR", dir)
        .env_remove("GODMESH_GRANTS")
        .env_remove("BIND_ADDR")
        .output()
        .expect("binário de teste executa")
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Linhas do controle: nome, checksum e instante de aplicação em texto (sem
/// depender de feature de data do sqlx).
async fn control_rows(pool: &PgPool) -> Vec<(String, String, String)> {
    sqlx::query_as("SELECT nome, checksum, aplicado_em::text FROM schema_migration ORDER BY nome")
        .fetch_all(pool)
        .await
        .expect("leitura de schema_migration")
}

/// GET HTTP/1.1 cru: prova no fio, não só a função de handler.
fn http_get(addr: SocketAddr, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .expect("envia o pedido");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("lê a resposta");
    let status: u16 = raw
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("linha de status HTTP");
>>>>>>> 034b10b (feat(server): migra na inicialização, healthz com dependência e shutdown gracioso)
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    (status, body)
}

<<<<<<< HEAD
fn json(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|e| panic!("corpo não é JSON ({e}): {raw:?}"))
}

async fn create(pool: &PgPool, addr: SocketAddr, id: &str) -> Value {
    let _ = pool;
    let token = format!("Bearer {TOKEN_A}");
    let id_owned = id.to_owned();
    let body = format!("{{\"incident_id\":\"{id_owned}\"}}");
    let (status, raw) =
        tokio::task::spawn_blocking(move || http(addr, "POST", "/v1/incidents", Some(&token), Some(&body)))
            .await
            .expect("cliente HTTP");
    assert_eq!(status, 201, "criação precisa ser 201: {raw}");
    json(&raw)
}

async fn call(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: &str,
    body: Option<String>,
) -> (u16, Value) {
    let token = format!("Bearer {token}");
    let path = path.to_owned();
    let method = method.to_owned();
    let (status, raw) = tokio::task::spawn_blocking(move || {
        http(addr, &method, &path, Some(&token), body.as_deref())
    })
    .await
    .expect("cliente HTTP");
    (status, json(&raw))
}

fn ack_classes(view: &Value) -> Vec<String> {
    view["acks"]
        .as_array()
        .expect("acks é lista")
        .iter()
        .map(|a| a["ack_class"].as_str().expect("ack_class string").to_owned())
        .collect()
}

// --- caminho completo feliz---------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_happy_path_derives_every_label_from_the_state() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("feliz");

    let created = create(&pool, addr, &id).await;
    assert_eq!(created["state"], "persisted_local");
    assert_eq!(created["ack_class"], "persisted_local");

    let (status, got) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(status, 200, "GET do próprio incidente: {got}");
    assert_eq!(got["incident_id"], id);
    assert_eq!(ack_classes(&got), vec!["persisted_local".to_owned()]);

    let (status, replicated) =
        call(addr, "POST", &format!("/v1/incidents/{id}/replicate"), TOKEN_A, None).await;
    assert_eq!(status, 200, "replicate do estado persistido: {replicated}");
    assert_eq!(replicated["state"], "replicated_to_authority");
    assert_eq!(replicated["ack_class"], "replicated_to_authority");
    assert_eq!(replicated["ui_label"], "replicado à autoridade, sem confirmação");

    let (status, technical) = call(
        addr,
        "POST",
        &format!("/v1/incidents/{id}/ack-technical"),
        TOKEN_A,
        None,
    )
    .await;
    assert_eq!(status, 200, "ack-technical: {technical}");
    assert_eq!(technical["state"], "acknowledged_technical");
    assert_eq!(technical["ack_class"], "materialized");

    let (status, human) = call(
        addr,
        "POST",
        &format!("/v1/incidents/{id}/acknowledge"),
        TOKEN_A,
        Some("{\"responder\":\"maria-plantonista\"}".to_owned()),
    )
    .await;
    assert_eq!(status, 200, "acknowledge com pessoa: {human}");
    assert_eq!(human["state"], "acknowledged_human");
    assert_eq!(human["ack_class"], "acknowledged_human");
    assert!(
        !human["ui_label"].as_str().unwrap_or_default().contains("entrega"),
        "rótulo não promete entrega: {}",
        human["ui_label"]
    );

    let (status, closed) =
        call(addr, "POST", &format!("/v1/incidents/{id}/close"), TOKEN_A, None).await;
    assert_eq!(status, 200, "close a partir do ACK humano: {closed}");
    assert_eq!(closed["state"], "closed");
    // O ACK final permanece: encerrar não apaga nem reescreve evidência.
    assert_eq!(closed["ack_class"], "acknowledged_human");

    let (status, final_view) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(status, 200);
    assert_eq!(
        ack_classes(&final_view),
        vec![
            "persisted_local".to_owned(),
            "replicated_to_authority".to_owned(),
            "materialized".to_owned(),
            "acknowledged_human".to_owned(),
        ],
        "trilha append-only na ordem das transições"
    );
    for ack in final_view["acks"].as_array().expect("acks") {
        let at = ack["at_unix"].as_i64().expect("at_unix inteiro");
        assert!(at > 0, "ACK sem instante real não é evidência: {ack}");
    }
    assert_eq!(
        final_view["acks"][3]["source"], "maria-plantonista",
        "ACK humano registra a pessoa que reconheceu"
    );
    assert_eq!(final_view["acks"][1]["source"], "authority:godmesh-server");
}

// --- transição fora de ordem ------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transition_out_of_order_is_409_with_the_current_state() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("fora-de-ordem");
    create(&pool, addr, &id).await;

    // Pular direto para o reconhecimento humano a partir de `persisted_local`.
    let (status, body) = call(
        addr,
        "POST",
        &format!("/v1/incidents/{id}/acknowledge"),
        TOKEN_A,
        Some("{\"responder\":\"maria-plantonista\"}".to_owned()),
    )
    .await;
    assert_eq!(status, 409, "transição ilegal precisa ser 409: {body}");
    assert_eq!(body["error"], "illegal_transition");
    assert_eq!(body["state"], "persisted_local", "corpo carrega o estado atual");
    assert_eq!(body["ack_class"], "transport_accepted", "erro não alega ACK");

    // Nada mudou: o estado e a trilha continuam os de antes da recusa.
    let (_, view) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(view["state"], "persisted_local");
    assert_eq!(ack_classes(&view), vec!["persisted_local".to_owned()]);
}

// --- prova de pessoa obrigatória -------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acknowledge_without_responder_is_422_missing_human_proof() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("sem-prova");
    create(&pool, addr, &id).await;
    let (s, _) = call(addr, "POST", &format!("/v1/incidents/{id}/replicate"), TOKEN_A, None).await;
    assert_eq!(s, 200);
    let (s, _) = call(addr, "POST", &format!("/v1/incidents/{id}/ack-technical"), TOKEN_A, None).await;
    assert_eq!(s, 200);

    for body in [
        Some("{}".to_owned()),
        Some("{\"responder\":\"   \"}".to_owned()),
        None,
    ] {
        let (status, refused) = call(
            addr,
            "POST",
            &format!("/v1/incidents/{id}/acknowledge"),
            TOKEN_A,
            body.clone(),
        )
        .await;
        assert_eq!(status, 422, "sem prova de pessoa precisa ser 422 ({body:?}): {refused}");
        assert_eq!(refused["error"], "missing_human_proof");
    }

    let (_, view) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(view["state"], "acknowledged_technical", "recusa não transiciona");
    assert_eq!(
        ack_classes(&view),
        vec![
            "persisted_local".to_owned(),
            "replicated_to_authority".to_owned(),
            "materialized".to_owned(),
        ],
        "recusa não grava ACK humano"
    );
}

// --- isolamento entre tenants ----------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incident_of_another_tenant_is_404_not_403() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    declare_tenant(&pool, TENANT_B, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("alheio");
    create(&pool, addr, &id).await;

    let (status, body) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_B, None).await;
    assert_eq!(status, 404, "incidente de outro tenant não vaza existência: {body}");
    assert_eq!(body["error"], "incident_not_found");
    assert_ne!(status, 403, "403 confirmaria a existência do incidente alheio");

    let (status, body) =
        call(addr, "POST", &format!("/v1/incidents/{id}/replicate"), TOKEN_B, None).await;
    assert_eq!(status, 404, "transição de outro tenant também é 404: {body}");

    // Inexistente responde exatamente o mesmo.
    let missing = unique_id("inexistente");
    let (status, body) = call(addr, "GET", &format!("/v1/incidents/{missing}"), TOKEN_A, None).await;
    assert_eq!(status, 404, "inexistente é 404: {body}");
    assert_eq!(body["error"], "incident_not_found");

    // O dono segue intacto.
    let (status, view) = call(addr, "GET", &format!("/v1/incidents/{id}"), TOKEN_A, None).await;
    assert_eq!(status, 200);
    assert_eq!(view["state"], "persisted_local", "nenhuma tentativa alheia transiciona");
}

// --- concorrência -----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_same_transition_yields_exactly_one_success_and_one_409() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("corrida");
    create(&pool, addr, &id).await;

    let path = format!("/v1/incidents/{id}/replicate");
    let one = {
        let path = path.clone();
        let token = format!("Bearer {TOKEN_A}");
        tokio::task::spawn_blocking(move || http(addr, "POST", &path, Some(&token), None))
    };
    let two = {
        let path = path.clone();
        let token = format!("Bearer {TOKEN_A}");
        tokio::task::spawn_blocking(move || http(addr, "POST", &path, Some(&token), None))
    };
    let (first, second) = tokio::join!(one, two);
    let (first, second) = (first.expect("1ª chamada"), second.expect("2ª chamada"));
    let mut statuses = vec![first.0, second.0];
    statuses.sort_unstable();
    assert_eq!(
        statuses,
        vec![200, 409],
        "exatamente uma sucesso e uma 409: {first:?} / {second:?}"
    );
    let conflict = if first.0 == 409 { &first.1 } else { &second.1 };
    assert!(
        conflict.contains("illegal_transition"),
        "conflito precisa ser nomeado: {conflict}"
    );

    // A trilha tem exatamente uma linha `replicated_to_authority`: o ACK não
    // duplica sob concorrência.
    let acks: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sos_ack WHERE incident_id = $1 AND ack_class = 'replicated_to_authority'",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .expect("contagem de ACK");
    assert_eq!(acks, 1, "ACK duplicado sob concorrência");

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM sos_ack WHERE incident_id = $1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .expect("contagem de ACK");
    assert_eq!(total, 2, "persisted_local + replicated_to_authority, nada mais");
}

// --- trilha append-only ----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ack_trail_is_append_only_and_records_real_instants() {
    let _guard = DB_LOCK.lock().await;
    let Some(pool) = pool().await else { eprintln!("SKIP: DATABASE_URL ausente"); return };
    declare_tenant(&pool, TENANT_A, "BR").await;
    let addr = serve(pool.clone()).await;
    let id = unique_id("append-only");
    create(&pool, addr, &id).await;
    let (s, _) = call(addr, "POST", &format!("/v1/incidents/{id}/replicate"), TOKEN_A, None).await;
    assert_eq!(s, 200);

    let rows: Vec<(String, i64)> =
        sqlx::query_as("SELECT ack_class, at_unix FROM sos_ack WHERE incident_id = $1 ORDER BY recorded_at, at_unix, ack_class")
            .bind(&id)
            .fetch_all(&pool)
            .await
            .expect("trilha");
    assert_eq!(rows.len(), 2, "duas fronteiras atravessadas, duas linhas");
    assert_eq!(rows[0].0, "persisted_local");
    assert_eq!(rows[1].0, "replicated_to_authority");
    for (class, at) in &rows {
        assert!(*at > 0, "at_unix real obrigatório em {class}");
    }
    assert!(
        rows[0].1 <= rows[1].1,
        "instantes não podem retroceder: {rows:?}"
    );

    // O banco recusa reescrever a trilha (trigger `sos_ack_append_only`).
    let update = sqlx::query("UPDATE sos_ack SET at_unix = at_unix + 1 WHERE incident_id = $1")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(update.is_err(), "UPDATE em sos_ack precisa ser recusado pelo banco");
    let delete = sqlx::query("DELETE FROM sos_ack WHERE incident_id = $1")
        .bind(&id)
        .execute(&pool)
        .await;
    assert!(delete.is_err(), "DELETE em sos_ack precisa ser recusado pelo banco");
}
=======
/// Sobe o router real em porta efêmera, sem shutdown gracioso: estes testes
/// olham `/healthz`, não o encerramento.
async fn serve_router(app: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn empty_grants() -> GrantRegistry {
    GrantRegistry::from_json("{\"grants\":[]}").expect("registro vazio é válido")
}

async fn pool_for(url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(url)
        .await
        .expect("pool do banco de teste")
}

// --- 1. migração na inicialização ---------------------------------------------

#[tokio::test]
async fn migracao_roda_duas_vezes_sem_duplicar_e_sem_reescrever_aplicado_em() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;
    let url = sa_url(&base);
    let dir = migrations_dir();

    let primeira = run_migrate_only(&url, &dir);
    assert!(
        primeira.status.success(),
        "1ª execução falhou: {}\n{}",
        primeira.status,
        stderr_of(&primeira)
    );

    let pool = pool_for(&url).await;
    let depois_da_primeira = control_rows(&pool).await;
    let arquivos = nomes_de_migracao(&dir);
    assert_eq!(
        depois_da_primeira
            .iter()
            .map(|(nome, _, _)| nome.clone())
            .collect::<Vec<_>>(),
        arquivos,
        "o controle deve listar exatamente as migrações do diretório, em ordem"
    );
    for (nome, checksum, _) in &depois_da_primeira {
        assert_eq!(checksum.len(), 64, "{nome}: checksum sha256 em hex");
    }

    let segunda = run_migrate_only(&url, &dir);
    assert!(
        segunda.status.success(),
        "2ª execução (idempotente) falhou: {}\n{}",
        segunda.status,
        stderr_of(&segunda)
    );
    let saida = stdout_of(&segunda);
    assert!(
        saida.contains("0 aplicada(s)") && saida.contains("já aplicada(s)"),
        "2ª execução deveria relatar tudo como já aplicado:\n{saida}"
    );

    let depois_da_segunda = control_rows(&pool).await;
    assert_eq!(
        depois_da_primeira, depois_da_segunda,
        "rodar de novo não pode duplicar linha nem reescrever aplicado_em"
    );
    pool.close().await;
}

#[tokio::test]
async fn migracao_com_checksum_divergente_falha_alto() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;
    let url = sa_url(&base);
    let dir = migrations_dir();

    assert!(
        run_migrate_only(&url, &dir).status.success(),
        "migração inicial"
    );

    let pool = pool_for(&url).await;
    let nome = nomes_de_migracao(&dir)
        .into_iter()
        .next()
        .expect("ao menos uma migração");
    sqlx::query("UPDATE schema_migration SET checksum = $1 WHERE nome = $2")
        .bind("0".repeat(64))
        .bind(&nome)
        .execute(&pool)
        .await
        .expect("adultera o checksum registrado");

    let tentativa = run_migrate_only(&url, &dir);
    assert!(
        !tentativa.status.success(),
        "checksum divergente precisa falhar o processo, não passar em silêncio:\n{}",
        stdout_of(&tentativa)
    );
    let erro = stderr_of(&tentativa);
    assert!(
        erro.contains(&nome) && erro.contains("checksum"),
        "erro precisa nomear a migração e o checksum:\n{erro}"
    );

    // O histórico não é "consertado" pelo runner: nada além da adulteração mudou.
    let linhas = control_rows(&pool).await;
    assert_eq!(linhas.len(), nomes_de_migracao(&dir).len());
    pool.close().await;
}

#[tokio::test]
async fn migracao_quebrada_falha_alto_e_nao_grava_controle() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;
    let url = sa_url(&base);
    let dir = migrations_dir();

    assert!(
        run_migrate_only(&url, &dir).status.success(),
        "migração inicial"
    );
    let pool = pool_for(&url).await;
    let antes = control_rows(&pool).await;

    // Diretório alternativo: prova também que GODMESH_MIGRATIONS_DIR é honrado.
    let temp = std::env::temp_dir().join(format!(
        "godmesh-mig-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio do sistema")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("diretório temporário de migrações");
    std::fs::write(
        temp.join("9999_quebrada.sql"),
        "CREATE TABLE tabela_que_nao_deve_existir (id int NOT NULL PRIMARY KEY);\n\
         CREATE TABLE tabela_que_nao_deve_existir (id int);\n",
    )
    .expect("escreve migração quebrada");

    let tentativa = run_migrate_only(&url, &temp);
    let _ = std::fs::remove_dir_all(&temp);
    assert!(
        !tentativa.status.success(),
        "migração que quebra precisa derrubar o processo:\n{}",
        stdout_of(&tentativa)
    );
    let erro = stderr_of(&tentativa);
    assert!(
        erro.contains("9999_quebrada.sql"),
        "erro precisa nomear o arquivo:\n{erro}"
    );

    // Bloco transacional implícito: nem a tabela do script nem a linha de
    // controle sobreviveram à falha.
    let tabela: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.tabela_que_nao_deve_existir')::text")
            .fetch_one(&pool)
            .await
            .expect("consulta to_regclass");
    assert!(
        tabela.is_none(),
        "migração que falhou deixou objeto no banco"
    );
    assert_eq!(
        control_rows(&pool).await,
        antes,
        "controle mudou após falha"
    );
    pool.close().await;
}

// --- 2. /healthz com dependência real -----------------------------------------

// Multi-thread de propósito: o cliente HTTP abaixo é bloqueante e precisa de
// outra worker para o servidor em processo responder.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthz_e_200_com_banco_e_503_com_banco_derrubado_e_volta() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;
    let url = sa_url(&base);
    let dir = migrations_dir();
    let migrou = run_migrate_only(&url, &dir);
    assert!(migrou.status.success(), "{}", stderr_of(&migrou));

    let pool = pool_for(&url).await;
    let addr = serve_router(http::router(pool.clone(), empty_grants())).await;

    let (status, body) = http_get(addr, "/healthz");
    assert_eq!(status, 200, "banco ok precisa dar 200; corpo: {body}");
    assert!(body.contains("\"status\":\"ok\""), "corpo: {body}");
    assert!(
        body.contains("\"ack_class\":\"transport_accepted\""),
        "ack_class sai nas duas respostas; corpo: {body}"
    );

    // Banco indisponível: fecha a porta do banco e derruba as sessões abertas.
    let db = sa_db();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER DATABASE {db} ALLOW_CONNECTIONS false"
    )))
    .execute(&admin)
    .await
    .expect("desabilita conexões ao banco de teste");
    let terminadas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM (SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1) AS t")
            .bind(&db)
            .fetch_one(&admin)
            .await
            .expect("termina sessões no banco de teste");
    assert!(terminadas >= 0, "contagem de sessões terminadas");

    let (status, body) = http_get(addr, "/healthz");
    // Restaura antes de qualquer asserção que possa abortar o teste.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER DATABASE {db} ALLOW_CONNECTIONS true"
    )))
    .execute(&admin)
    .await
    .expect("reabilita conexões ao banco de teste");

    assert_eq!(status, 503, "banco fora precisa dar 503; corpo: {body}");
    assert!(
        body.contains("\"status\":\"unavailable\""),
        "corpo precisa declarar indisponibilidade: {body}"
    );
    assert!(
        body.contains("\"dependency\":\"postgres\""),
        "corpo precisa nomear a dependência: {body}"
    );
    assert!(
        body.contains("\"ack_class\":\"transport_accepted\""),
        "ack_class se mantém no 503; corpo: {body}"
    );

    // Recuperação: o mesmo processo volta a responder 200 sem reiniciar.
    let mut ultimo = (0u16, String::new());
    for _ in 0..50 {
        ultimo = http_get(addr, "/healthz");
        if ultimo.0 == 200 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        ultimo.0, 200,
        "não voltou a 200 após religar o banco: {}",
        ultimo.1
    );
    pool.close().await;
}

// --- 3. shutdown gracioso -----------------------------------------------------

/// O `serve` do binário: requisição em voo termina depois do sinal, e o pool
/// fecha. Prova a drenagem, que um `drop` de runtime não faria.
#[tokio::test]
async fn shutdown_drena_requisicao_em_voo_e_fecha_o_pool() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    ensure_sa(&admin).await;
    let pool = pool_for(&sa_url(&base)).await;

    // Rota lenta: mantém a requisição em voo enquanto o sinal chega.
    let app = Router::new().route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_millis(400)).await;
            "drenada"
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let (sinal, espera) = tokio::sync::oneshot::channel::<()>();

    let servidor = tokio::spawn(lifecycle::serve(listener, app, pool.clone(), async move {
        let _ = espera.await;
    }));

    // Cliente começa a requisição antes do sinal e só termina depois dele.
    let cliente = tokio::task::spawn_blocking(move || http_get(addr, "/slow"));
    tokio::time::sleep(Duration::from_millis(150)).await;
    sinal.send(()).expect("envia o sinal de shutdown");

    let relatorio = servidor
        .await
        .expect("tarefa do servidor")
        .expect("serve sem erro de I/O");
    assert_eq!(
        relatorio.drained, 1,
        "a requisição em voo no instante do sinal precisa ser contada como drenada"
    );

    let (status, body) = cliente.await.expect("cliente HTTP");
    assert_eq!(
        status, 200,
        "requisição em voo precisa terminar com sucesso"
    );
    assert_eq!(body, "drenada");

    // Pool fechado: nenhuma conexão nova, e a consulta passa a falhar.
    let erro = sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .expect_err("pool precisa estar fechado depois do shutdown");
    assert!(
        erro.to_string().contains("closed"),
        "erro deveria dizer que o pool está fechado: {erro}"
    );
}

/// O binário de verdade: SIGTERM encerra com código 0 e imprime a contagem
/// drenada. Sem isso, o sinal só existiria no teste em processo.
#[cfg(unix)]
#[tokio::test]
async fn binario_encerra_por_sigterm_com_codigo_zero() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;

    let mut filho = Command::new(binario())
        .env("DATABASE_URL", sa_url(&base))
        .env("GODMESH_MIGRATIONS_DIR", migrations_dir())
        .env("GODMESH_GRANTS", "{\"grants\":[]}")
        .env("GODMESH_ENV", "dev")
        .env("BIND_ADDR", "127.0.0.1:0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sobe o binário de teste");

    let mut log = String::new();
    let mut linhas = BufReader::new(filho.stdout.take().expect("stdout")).lines();
    let mut escutando = false;
    for _ in 0..50 {
        match linhas.next() {
            Some(Ok(linha)) => {
                log.push_str(&linha);
                log.push('\n');
                if linha.contains("escutando em") {
                    escutando = true;
                    break;
                }
            }
            Some(Err(e)) => panic!("stdout ilegível: {e}"),
            None => break,
        }
    }
    assert!(escutando, "o binário não chegou a escutar:\n{log}");

    let pid = filho.id();
    let sinal = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("envia SIGTERM");
    assert!(sinal.success(), "kill -TERM falhou: {sinal}");

    let mut codigo = None;
    for _ in 0..100 {
        if let Some(status) = filho.try_wait().expect("try_wait") {
            codigo = Some(status.code());
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let codigo = codigo.expect("processo não encerrou em 10s após SIGTERM");
    for linha in linhas.map_while(Result::ok) {
        log.push_str(&linha);
        log.push('\n');
    }
    assert_eq!(codigo, Some(0), "SIGTERM precisa sair com 0; log:\n{log}");
    assert!(
        log.contains("em voo drenada"),
        "log precisa trazer a contagem drenada:\n{log}"
    );
    assert!(
        log.contains("encerrado"),
        "log precisa registrar o encerramento:\n{log}"
    );
}

/// O runner é o dono da transação: nenhum arquivo de migração abre `BEGIN`.
#[test]
fn migracoes_nao_abrem_transacao_propria() {
    for nome in nomes_de_migracao(&migrations_dir()) {
        let sql = std::fs::read_to_string(migrations_dir().join(&nome)).expect("lê migração");
        for (indice, linha) in sql.lines().enumerate() {
            let sem_espaco = linha.trim().to_ascii_lowercase();
            assert!(
                !(sem_espaco == "begin;" || sem_espaco == "commit;"),
                "{nome}:{}: o runner é dono da transação; remova {linha:?}",
                indice + 1
            );
        }
    }
}

/// O esquema de controle é o documentado (`schema_migration` v1).
#[test]
fn esquema_de_migracao_e_v1() {
    assert_eq!(migrate::SCHEMA_VERSION, "schema_migration v1");
    assert_eq!(migrate::DEFAULT_MIGRATIONS_DIR, "migrations");
    assert_eq!(migrate::MIGRATIONS_DIR_ENV, "GODMESH_MIGRATIONS_DIR");
}
>>>>>>> 034b10b (feat(server): migra na inicialização, healthz com dependência e shutdown gracioso)
