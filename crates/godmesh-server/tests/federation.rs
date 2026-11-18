//! Federação — identidade, manifesto assinado e ingresso por API (ADR-071, M1).
//!
//! O alvo é o portão HTTP real (`godmesh_server::http::router_with`) contra
//! Postgres de verdade, com banco **descartável** migrado pelo runner oficial
//! (`tests/common/mod.rs`). Cada passo confere o **efeito** (linha no banco,
//! estado de confiança, ausência de peer), não só o status.
//!
//! O que esta suíte prova:
//!
//! * `POST /v1/federation/join-requests` é público (sem `Authorization`) e só
//!   aceita manifesto com assinatura Ed25519 válida e `issued_at` recente;
//! * ingresso válido cria `pending` — **não** cria par autorizado;
//! * aprovar cria/atualiza `federation_peer` como `authorized`; rejeitar não
//!   cria peer nenhum;
//! * idempotência por `(node_id, epoch)`: reenviar não duplica nem reescreve
//!   `requested_at`;
//! * chave privada nunca aparece em resposta nem em log: o hex da semente do
//!   arquivo de teste é procurado em todos os corpos e linhas de auditoria.
//!
//! Execução (local-first, ADR-070 — sem contêiner):
//!
//! ```text
//! DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
//!   cargo test -p godmesh-server --test federation -- --nocapture
//! ```

mod common;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use godmesh_federation::identity::NodeIdentity;
use godmesh_federation::manifest::SignedManifest;
use godmesh_federation::peer::TrustState;
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{
    Authorization, CapturedAudit, GrantRecord, GrantRegistry, PgRevocationSource, Scope, Subject,
    SystemClock,
};
use godmesh_server::http::router_with_log;
use godmesh_server::observability::CapturedLog;
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, PgPool};

/// Token com `federation:admin` (aprovar/rejeitar).
const TOKEN_ADMIN: &str = "token-federation-admin";
/// Token com apenas `federation:read` (listar).
const TOKEN_READ: &str = "token-federation-read";
/// Token sem nenhum escopo de federação.
const TOKEN_SEM_FEDERACAO: &str = "token-sem-federacao";

// --- ambiente ---------------------------------------------------------------

struct Env {
    url: String,
}

static ENV: tokio::sync::OnceCell<Env> = tokio::sync::OnceCell::const_new();

fn db_name() -> String {
    format!("godmesh_federation_{}", std::process::id())
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
    let admin = sqlx::postgres::PgPoolOptions::new()
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
    // Migra pelo runner oficial; o pool é descartado e recriado por teste.
    common::pool_para(&url, 4).await.close().await;
    eprintln!("federation: banco descartável pronto em {url}");
    Env { url }
}

async fn env() -> Option<&'static Env> {
    if common::database_url().is_none() {
        eprintln!(
            "SKIP: sem GODMESH_TEST_DATABASE_URL/DATABASE_URL; a suíte de federação depende de Postgres"
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
            tenant_id: "tenant-federation-br".to_owned(),
            principal: "operador-federacao".to_owned(),
        },
        scopes,
        not_before: 0,
        expires_at: i64::MAX,
        revoked_at: None,
        authority_epoch: 11,
        predicate: predicate(),
    }
}

fn registry() -> GrantRegistry {
    GrantRegistry::from_records([
        (
            TOKEN_ADMIN.to_owned(),
            record("g-fed-admin", vec![Scope::FederationAdmin]),
        ),
        (
            TOKEN_READ.to_owned(),
            record("g-fed-read", vec![Scope::FederationRead]),
        ),
        (
            TOKEN_SEM_FEDERACAO.to_owned(),
            record("g-fed-none", vec![Scope::IncidentRead]),
        ),
    ])
}

fn authorization(pool: &PgPool, audit: Arc<CapturedAudit>) -> Authorization {
    Authorization::with_parts(
        registry(),
        Arc::new(SystemClock),
        Arc::new(PgRevocationSource::new(pool.clone())),
        // TTL irrelevante aqui: nenhum teste de federação revoga grant.
        5,
        audit,
    )
}

struct Server {
    addr: std::net::SocketAddr,
    audit: Arc<CapturedAudit>,
    log: Arc<CapturedLog>,
}

async fn serve(pool: PgPool) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let audit = Arc::new(CapturedAudit::default());
    let log = Arc::new(CapturedLog::new());
    let authorization = authorization(&pool, audit.clone());
    let app = router_with_log(pool, authorization, log.clone());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Server { addr, audit, log }
}

// --- identidade e manifesto -------------------------------------------------

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("relógio do sistema depois da época")
        .as_secs() as i64
}

/// Diretório de chave por teste, para que a semente seja única e o arquivo
/// `0600` seja exercitado de verdade.
fn key_path(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("godmesh-federation-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir de chave");
    dir.join("node.key")
}

struct Node {
    identity: NodeIdentity,
    /// Hex da semente, lido do arquivo que o próprio teste criou. Serve **só**
    /// para provar que o servidor nunca devolve nem registra esse valor.
    seed_hex: String,
}

fn node_with_key(tag: &str) -> Node {
    let path = key_path(tag);
    let identity = NodeIdentity::load_or_create(&path).expect("identidade do nó");
    let seed_hex = std::fs::read_to_string(&path)
        .expect("arquivo de chave")
        .trim()
        .to_owned();
    assert_eq!(seed_hex.len(), 64, "semente em hex de 32 bytes");
    Node { identity, seed_hex }
}

fn manifest_for(node: &Node, epoch: u64, issued_at: i64) -> SignedManifest {
    let mut manifest = SignedManifest {
        node_id: String::new(),
        public_key: String::new(),
        cluster: "cluster-br-sul".to_owned(),
        region: "br-sul".to_owned(),
        software_version: "0.1.0".to_owned(),
        alpn: String::new(),
        capabilities: vec!["sync".to_owned(), "events".to_owned()],
        endpoints: vec!["quic://10.0.0.1:7000".to_owned()],
        epoch,
        issued_at,
        signature: String::new(),
    };
    manifest.sign(&node.identity);
    manifest
}

// --- consultas de verificação ----------------------------------------------

async fn count_joins(pool: &PgPool, node_id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM federation_join_request WHERE node_id = $1")
        .bind(node_id)
        .fetch_one(pool)
        .await
        .expect("conta pedidos")
}

async fn peer_state(pool: &PgPool, node_id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT trust_state FROM federation_peer WHERE node_id = $1")
        .bind(node_id)
        .fetch_optional(pool)
        .await
        .expect("lê trust_state do par")
}

// --- pedido de ingresso -----------------------------------------------------

fn join(server: &Server, manifest: &SignedManifest) -> common::HttpResponse {
    let body = serde_json::to_string(manifest).expect("manifesto serializa");
    common::call(
        server.addr,
        "POST",
        "/v1/federation/join-requests",
        None,
        Some(&body),
    )
}

/// Cria um pedido pending válido e devolve o JSON da resposta.
fn join_pending(server: &Server, node: &Node, epoch: u64) -> Value {
    let manifest = manifest_for(node, epoch, now_unix());
    let res = join(server, &manifest);
    assert_eq!(
        res.status, 201,
        "ingresso válido precisa ser 201: {}",
        res.body
    );
    res.json()
}

// --- 1) manifesto inválido --------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn join_sem_assinatura_valida_e_recusado_sem_gravar() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("assinatura");
    let outro = node_with_key("assinatura-outro");
    let manifest = manifest_for(&node, 1, now_unix());

    // (a) assinatura de outra chave: bytes válidos para `outro`, não para `node`.
    let mut assinado_por_outro = manifest.clone();
    assinado_por_outro.signature = outro
        .identity
        .sign_hex(&assinado_por_outro.canonical_bytes());
    let res = join(&server, &assinado_por_outro);
    assert_eq!(res.status, 401, "assinatura alheia: {}", res.body);
    assert_eq!(res.json()["error"], "invalid_manifest_signature");

    // (b) assinatura adulterada depois de assinar.
    let mut adulterado = manifest.clone();
    adulterado.cluster = "cluster-alvo".to_owned();
    let res = join(&server, &adulterado);
    assert_eq!(res.status, 401, "manifesto adulterado: {}", res.body);
    assert_eq!(res.json()["error"], "invalid_manifest_signature");

    // (c) `node_id` de outro nó com assinatura legítima da própria chave.
    let mut node_id_de_outro = manifest.clone();
    node_id_de_outro.node_id = outro.identity.node_id().to_owned();
    // Assinado com a chave certa (a do nó), mas declarando o `node_id` alheio:
    // a assinatura confere e o vínculo `node_id`↔chave é que não fecha.
    node_id_de_outro.signature = node.identity.sign_hex(&node_id_de_outro.canonical_bytes());
    let res = join(&server, &node_id_de_outro);
    assert_eq!(res.status, 422, "node_id de outro nó: {}", res.body);
    assert_eq!(res.json()["error"], "invalid_manifest");

    // Nada foi gravado: nem pedido, nem par.
    assert_eq!(count_joins(&pool, node.identity.node_id()).await, 0);
    assert_eq!(count_joins(&pool, outro.identity.node_id()).await, 0);
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());
    assert!(peer_state(&pool, outro.identity.node_id()).await.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn manifesto_com_issued_at_velho_e_recusado() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("expirado");
    // Uma hora de idade: fora da janela padrão de 5 minutos.
    let manifest = manifest_for(&node, 1, now_unix() - 3_600);
    let res = join(&server, &manifest);
    assert_eq!(res.status, 422, "manifesto velho: {}", res.body);
    assert_eq!(res.json()["error"], "manifest_expired");
    assert_eq!(count_joins(&pool, node.identity.node_id()).await, 0);

    // O mesmo manifesto com instante recente passa: a recusa é de frescor, não de
    // assinatura.
    let recente = manifest_for(&node, 1, now_unix());
    let res = join(&server, &recente);
    assert_eq!(res.status, 201, "manifesto recente: {}", res.body);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn manifesto_emitido_no_futuro_e_recusado() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("futuro");
    let manifest = manifest_for(&node, 1, now_unix() + 3_600);
    let res = join(&server, &manifest);
    assert_eq!(res.status, 422, "manifesto futuro: {}", res.body);
    assert_eq!(res.json()["error"], "manifest_not_yet_valid");
    assert_eq!(count_joins(&pool, node.identity.node_id()).await, 0);
}

// --- 2) ingresso pending ----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn join_valido_cria_pending_e_nao_autoriza_par() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("pending");
    let body = join_pending(&server, &node, 1);
    assert_eq!(body["state"], "pending");
    assert_eq!(body["node_id"], node.identity.node_id());
    assert!(
        body["join_request_id"]
            .as_str()
            .expect("id")
            .starts_with("fjr_")
    );
    assert!(body["approved_at"].is_null());
    assert!(body["approved_by"].is_null());
    // O manifesto volta com a chave pública, nunca com segredo.
    assert_eq!(
        body["manifest"]["public_key"],
        node.identity.public_key_hex()
    );

    // Nó pending **não** é par autorizado: a linha de peer nem existe.
    assert_eq!(peer_state(&pool, node.identity.node_id()).await, None);
    assert_eq!(count_joins(&pool, node.identity.node_id()).await, 1);
}

// --- 3) aprovação e rejeição ------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aprovar_cria_peer_authorized() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("aprovar");
    let pending = join_pending(&server, &node, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();
    // Antes da aprovação não há par.
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());

    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 200, "aprovação: {}", res.body);
    let body = res.json();
    assert_eq!(body["state"], "approved");
    assert_eq!(body["approved_by"], "operador-federacao");
    assert!(body["approved_at"].as_i64().expect("instante de aprovação") > 0);

    // Efeito no banco: peer autorizado, com a chave pública do manifesto.
    assert_eq!(
        peer_state(&pool, node.identity.node_id()).await.as_deref(),
        Some("authorized")
    );

    // E a rota de leitura confirma, sem expor nada além de chave pública.
    let res = common::get(server.addr, "/v1/federation/peers", TOKEN_READ);
    assert_eq!(res.status, 200, "listagem de pares: {}", res.body);
    let peers = res.json();
    let list = peers.as_array().expect("lista de pares");
    let peer = list
        .iter()
        .find(|p| p["node_id"] == node.identity.node_id())
        .expect("par aprovado presente");
    assert_eq!(peer["trust_state"], "authorized");
    assert_eq!(peer["public_key"], node.identity.public_key_hex());
    assert!(peer.get("private_key").is_none());
    assert!(peer.get("secret").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rejeitar_nao_cria_peer() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("rejeitar");
    let pending = join_pending(&server, &node, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();

    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/reject"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 200, "rejeição: {}", res.body);
    assert_eq!(res.json()["state"], "rejected");

    // Nenhum peer foi criado por rejeição, e o pedido continua sem aprovação.
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());
    assert_eq!(count_joins(&pool, node.identity.node_id()).await, 1);

    // Aprovar depois de rejeitar é conflito, não caminho alternativo.
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 409, "aprovar rejeitado: {}", res.body);
    assert_eq!(res.json()["error"], "join_request_state_conflict");
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());

    // Rejeitar de novo também é conflito.
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/reject"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 409, "rejeitar de novo: {}", res.body);

    // Pedido inexistente é 404 nas duas decisões.
    for acao in ["approve", "reject"] {
        let res = common::call(
            server.addr,
            "POST",
            &format!("/v1/federation/join-requests/fjr_00000000000000000000000000000000/{acao}"),
            Some(TOKEN_ADMIN),
            None,
        );
        assert_eq!(res.status, 404, "{acao} inexistente: {}", res.body);
        assert_eq!(res.json()["error"], "join_request_not_found");
    }
}

// --- 4) idempotência --------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn join_duplicado_por_node_e_epoch_nao_duplica() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("idempotente");
    let manifest = manifest_for(&node, 4, now_unix());

    let primeiro = join(&server, &manifest);
    assert_eq!(primeiro.status, 201, "primeiro ingresso: {}", primeiro.body);
    let primeiro = primeiro.json();
    let requested_at = primeiro["requested_at"].as_i64().expect("instante");

    // Mesma geração, mesmo nó: 200 com o mesmo pedido, sem linha nova.
    let segundo = join(&server, &manifest);
    assert_eq!(segundo.status, 200, "replay: {}", segundo.body);
    let segundo = segundo.json();
    assert_eq!(segundo["join_request_id"], primeiro["join_request_id"]);
    assert_eq!(segundo["requested_at"], requested_at);
    assert_eq!(count_joins(&pool, node.identity.node_id()).await, 1);

    // Geração nova (epoch 5) é pedido novo, não replay.
    let proximo = manifest_for(&node, 5, now_unix());
    let terceiro = join(&server, &proximo);
    assert_eq!(terceiro.status, 201, "epoch novo: {}", terceiro.body);
    assert_ne!(
        terceiro.json()["join_request_id"],
        primeiro["join_request_id"]
    );
    assert_eq!(count_joins(&pool, node.identity.node_id()).await, 2);
}

// --- 5) escopo das rotas ----------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rotas_de_federacao_exigem_escopo() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("escopo");
    let pending = join_pending(&server, &node, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();

    // Sem Bearer nas rotas autenticadas: 401 (o pedido de ingresso é a exceção
    // pública, provada nos outros testes).
    for path in ["/v1/federation/join-requests", "/v1/federation/peers"] {
        let res = common::call(server.addr, "GET", path, None, None);
        assert_eq!(res.status, 401, "{path} sem token: {}", res.body);
        assert_eq!(res.json()["error"], "unauthenticated");
    }

    // Token sem escopo de federação: 403.
    for path in ["/v1/federation/join-requests", "/v1/federation/peers"] {
        let res = common::get(server.addr, path, TOKEN_SEM_FEDERACAO);
        assert_eq!(res.status, 403, "{path} sem escopo: {}", res.body);
        assert_eq!(res.json()["error"], "insufficient_scope");
    }

    // `federation:read` não aprova: ver não é decidir.
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_READ),
        None,
    );
    assert_eq!(res.status, 403, "read aprovando: {}", res.body);
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());

    // `federation:read` lista e filtra.
    let res = common::get(
        server.addr,
        "/v1/federation/join-requests?state=pending",
        TOKEN_READ,
    );
    assert_eq!(res.status, 200, "listagem: {}", res.body);
    let lista = res.json();
    assert!(
        lista
            .as_array()
            .expect("lista")
            .iter()
            .any(|r| r["join_request_id"] == id.as_str()),
        "pedido pending presente: {lista}"
    );

    // Estado de filtro fora do contrato é 422.
    let res = common::get(
        server.addr,
        "/v1/federation/join-requests?state=aprovado",
        TOKEN_READ,
    );
    assert_eq!(res.status, 422, "filtro inválido: {}", res.body);
    assert_eq!(res.json()["error"], "invalid_request");
}

// --- 6) segredo nunca sai ---------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chave_privada_nunca_aparece_em_resposta_nem_em_log() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("segredo");
    let seed = node.seed_hex.clone();
    assert_eq!(seed.len(), 64);

    let manifest = manifest_for(&node, 2, now_unix());
    let body = serde_json::to_string(&manifest).expect("manifesto serializa");
    // O próprio corpo enviado não tem a semente: só chave pública e assinatura.
    assert!(!body.contains(&seed), "pedido enviado contém a semente");

    let mut corpos = Vec::new();
    let res = join(&server, &manifest);
    assert_eq!(res.status, 201, "ingresso: {}", res.body);
    corpos.push(res.body.clone());
    let id = res.json()["join_request_id"]
        .as_str()
        .expect("id")
        .to_owned();

    for (metodo, path, token) in [
        (
            "GET",
            "/v1/federation/join-requests".to_owned(),
            Some(TOKEN_READ),
        ),
        ("GET", "/v1/federation/peers".to_owned(), Some(TOKEN_READ)),
        (
            "POST",
            format!("/v1/federation/join-requests/{id}/approve"),
            Some(TOKEN_ADMIN),
        ),
        ("GET", "/v1/federation/peers".to_owned(), Some(TOKEN_READ)),
    ] {
        let res = common::call(server.addr, metodo, &path, token, None);
        assert!(res.status < 300, "{metodo} {path}: {}", res.body);
        corpos.push(res.body);
    }

    // Nenhum corpo devolve a semente; a chave pública aparece, porque é pública.
    for corpo in &corpos {
        assert!(!corpo.contains(&seed), "resposta expôs a semente: {corpo}");
        assert!(
            !corpo.contains("private_key") && !corpo.contains("secret") && !corpo.contains("seed"),
            "resposta nomeia campo secreto: {corpo}"
        );
    }
    assert!(
        corpos
            .iter()
            .any(|c| c.contains(&node.identity.public_key_hex())),
        "a chave pública deveria aparecer (é dado público)"
    );

    // Auditoria e log de requisição: só identificadores públicos.
    let mut registros = server.audit.lines().await;
    registros.extend(server.log.lines());
    assert!(!registros.is_empty(), "houve auditoria/log");
    for linha in &registros {
        assert!(!linha.contains(&seed), "log expôs a semente: {linha}");
        // A semente é hex; a assinatura também. O que não pode aparecer é o
        // arquivo de chave nem o material secreto bruto.
        assert!(
            !linha.contains("node.key"),
            "log expôs o caminho da chave: {linha}"
        );
    }
    assert!(
        registros
            .iter()
            .any(|l| l.contains("event=federation_join_requested")),
        "auditoria do ingresso ausente: {registros:?}"
    );
    assert!(
        registros
            .iter()
            .any(|l| l.contains("event=federation_join_approved")),
        "auditoria da aprovação ausente: {registros:?}"
    );
}

// --- 7) estados de confiança não são inventados -----------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn trust_state_persistido_so_usa_o_contrato() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let server = serve(pool.clone()).await;

    let node = node_with_key("estado");
    let pending = join_pending(&server, &node, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 200, "aprovação: {}", res.body);

    // O banco só tem valores do contrato e o par está autorizado.
    let invalidos: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM federation_peer \
          WHERE trust_state NOT IN ('pending', 'authorized', 'revoked', 'unknown')",
    )
    .fetch_one(&pool)
    .await
    .expect("conta estados fora do contrato");
    assert_eq!(invalidos, 0);

    // Aprovar de novo é idempotente e continua authorized.
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 200, "aprovação repetida: {}", res.body);
    assert_eq!(
        peer_state(&pool, node.identity.node_id()).await.as_deref(),
        Some("authorized")
    );

    // Revogado no banco não volta a autorizado por aprovação de pedido pendente
    // de outra geração: a máquina de estados recusa (409).
    sqlx::query("UPDATE federation_peer SET trust_state = 'revoked' WHERE node_id = $1")
        .bind(node.identity.node_id())
        .execute(&pool)
        .await
        .expect("revoga o par direto no banco (simula decisão de revogação)");
    let novo = manifest_for(&node, 2, now_unix());
    let res = join(&server, &novo);
    assert_eq!(res.status, 201, "novo pedido após revogação: {}", res.body);
    let id = res.json()["join_request_id"]
        .as_str()
        .expect("id")
        .to_owned();
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 409, "revogado não volta direto: {}", res.body);
    assert_eq!(res.json()["error"], "illegal_trust_transition");
    assert_eq!(res.json()["current"], "revoked");
    assert_eq!(res.json()["next"], "authorized");
    assert_eq!(
        peer_state(&pool, node.identity.node_id()).await.as_deref(),
        Some("revoked"),
        "o par continua revogado"
    );

    // `TrustState` do crate concorda com o que o banco aceita.
    assert_eq!(TrustState::Authorized.as_str(), "authorized");
    assert_eq!(json!("pending").as_str(), Some("pending"));
}
