//! Federação — identidade, manifesto assinado, ingresso por API (ADR-071 M1) e
//! aprovação assinada por nó autorizado (emenda à ADR-071, fatia M1b).
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
//! * **M1b**: aprovar/rejeitar exigem duas autoridades compostas — o grant com
//!   `federation:admin` **e** um `ApprovalCertificate` assinado por nó
//!   autorizado (genesis declarado ou par `authorized` no registro). Certificado
//!   inválido, de chave não autorizada, expirado, com epoch regressivo ou
//!   apontando para outro pedido recusa com 422 e **sem transição de estado**;
//! * sem genesis e sem nenhum par `authorized`, aprovar é impossível — e é isso
//!   que a resposta diz, em vez de aceitar a primeira chave que aparecer;
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
use godmesh_federation::{ApprovalCertificate, DecisionAction, Genesis, GenesisNode};
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{
    Authorization, CapturedAudit, GrantRecord, GrantRegistry, PgRevocationSource, Scope, Subject,
    SystemClock,
};
use godmesh_server::federation::DecisionConfig;
use godmesh_server::http::router_with_log_and_decisions;
use godmesh_server::observability::CapturedLog;
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, PgPool, Row};

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

/// Servidor com governança declarada para o cluster de teste e sem genesis.
async fn serve(pool: PgPool) -> Server {
    serve_with_genesis(pool, Genesis::empty()).await
}

/// Servidor com o bootstrap de confiança injetado — os nós que podem aprovar
/// assinando. Injetado por parâmetro porque `GODMESH_FEDERATION_GENESIS` é
/// ambiente global e concorrente entre testes.
async fn serve_with_genesis(pool: PgPool, genesis: Genesis) -> Server {
    serve_with_governance(pool, genesis, Some("single_approver:cluster-br-sul")).await
}

async fn serve_with_governance(pool: PgPool, genesis: Genesis, governance: Option<&str>) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let audit = Arc::new(CapturedAudit::default());
    let log = Arc::new(CapturedLog::new());
    let authorization = authorization(&pool, audit.clone());
    let config = DecisionConfig::new(genesis, governance).expect("governança de teste");
    let app = router_with_log_and_decisions(pool, authorization, log.clone(), config);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Server { addr, audit, log }
}

/// Genesis a partir de nós de teste (identidade em arquivo `0600`).
fn genesis_of(nodes: &[&Node]) -> Genesis {
    Genesis::from_nodes(nodes.iter().map(|n| GenesisNode::from_identity(&n.identity)))
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
    manifest_for_cluster(node, epoch, issued_at, "cluster-br-sul")
}

/// Manifesto num cluster específico: os testes de bootstrap precisam de um
/// cluster onde **ninguém** foi autorizado — o banco é compartilhado pelos
/// testes do binário, então "nenhum aprovador" só é provável em cluster novo.
fn manifest_for_cluster(
    node: &Node,
    epoch: u64,
    issued_at: i64,
    cluster: &str,
) -> SignedManifest {
    let mut manifest = SignedManifest {
        node_id: String::new(),
        public_key: String::new(),
        cluster: cluster.to_owned(),
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

// --- certificado de aprovação (M1b) -----------------------------------------

/// Certificado assinado por `approver` sobre o pedido `pending` (resposta de
/// ingresso), com janela e epoch explícitas. `sign` preenche `node_id`,
/// `public_key` e `signature`: o teste não escolhe a identidade que assina.
#[allow(clippy::too_many_arguments)]
fn certificate_for(
    approver: &Node,
    pending: &Value,
    cluster: &str,
    not_before: i64,
    expires_at: i64,
    epoch: u64,
) -> ApprovalCertificate {
    let mut cert = ApprovalCertificate {
        join_request_id: pending["join_request_id"]
            .as_str()
            .expect("join_request_id")
            .to_owned(),
        node_id: String::new(),
        public_key: String::new(),
        cluster: cluster.to_owned(),
        scopes: vec!["federation:approve".to_owned()],
        not_before,
        expires_at,
        epoch,
        signature: String::new(),
    };
    cert.sign(&approver.identity);
    cert
}

/// Certificado válido agora, com 1h de validade e epoch 1.
fn fresh_certificate(approver: &Node, pending: &Value) -> ApprovalCertificate {
    certificate_for(approver, pending, "cluster-br-sul", now_unix() - 1, now_unix() + 3_600, 1)
}

fn decision_body(cert: &ApprovalCertificate) -> String {
    serde_json::json!({ "approval": cert }).to_string()
}

fn approve_with(server: &Server, id: &str, cert: &ApprovalCertificate) -> common::HttpResponse {
    common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_ADMIN),
        Some(&decision_body(cert)),
    )
}

fn reject_with(server: &Server, id: &str, cert: &ApprovalCertificate) -> common::HttpResponse {
    common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/reject"),
        Some(TOKEN_ADMIN),
        Some(&decision_body(cert)),
    )
}

/// Aprovador genesis padrão dos testes que só precisam de "uma decisão válida".
fn genesis_approver(tag: &str) -> Node {
    node_with_key(&format!("genesis-{tag}"))
}

// --- consultas de verificação ----------------------------------------------

/// A prova assinada como ficou no banco: (aprovador, assinatura, epoch, expira).
async fn stored_approval(pool: &PgPool, join_request_id: &str) -> (Option<String>, Option<String>, Option<i64>, Option<i64>) {
    let row = sqlx::query(
        "SELECT approved_by_node, approval_signature, approval_epoch, \
                extract(epoch from approval_expires_at)::bigint AS approval_expires_at \
           FROM federation_join_request WHERE join_request_id = $1",
    )
    .bind(join_request_id)
    .fetch_one(pool)
    .await
    .expect("lê a prova de aprovação persistida");
    (
        row.get("approved_by_node"),
        row.get("approval_signature"),
        row.get("approval_epoch"),
        row.get("approval_expires_at"),
    )
}

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
    join_pending_in(server, node, epoch, "cluster-br-sul")
}

fn join_pending_in(server: &Server, node: &Node, epoch: u64, cluster: &str) -> Value {
    let manifest = manifest_for_cluster(node, epoch, now_unix(), cluster);
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
    let approver = genesis_approver("aprovar");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&approver])).await;

    let node = node_with_key("aprovar");
    let pending = join_pending(&server, &node, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();
    // Antes da aprovação não há par.
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());

    let cert = fresh_certificate(&approver, &pending);
    let res = approve_with(&server, &id, &cert);
    assert_eq!(res.status, 200, "aprovação: {}", res.body);
    let body = res.json();
    assert_eq!(body["state"], "approved");
    // Duas autoridades compostas na resposta: o grant humano e o nó que assinou.
    // A assinatura devolvida é a **do certificado enviado** — comparar com o
    // próprio corpo seria uma asserção que não prova nada.
    assert_eq!(body["approved_by"], "operador-federacao");
    assert_eq!(body["approved_by_node"], approver.identity.node_id());
    assert_eq!(body["approval_signature"], cert.signature);
    assert_eq!(body["approval_epoch"], cert.epoch);
    assert_eq!(body["approval_expires_at"], cert.expires_at);
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
    let approver = genesis_approver("rejeitar");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&approver])).await;

    let node = node_with_key("rejeitar");
    let pending = join_pending(&server, &node, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();
    let cert = fresh_certificate(&approver, &pending);

    let res = reject_with(&server, &id, &cert);
    assert_eq!(
        res.status, 422,
        "prova de aprovação na rejeição: {}",
        res.body
    );
    assert_eq!(res.json()["error"], "approval_signature_invalid");
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());

    let mut rejection = cert.clone();
    rejection.sign_for(&approver.identity, DecisionAction::Reject);
    let res = reject_with(&server, &id, &rejection);
    assert_eq!(res.status, 200, "rejeição: {}", res.body);
    assert_eq!(res.json()["state"], "rejected");

    // Nenhum peer foi criado por rejeição, e o pedido continua sem aprovação —
    // inclusive sem prova de aprovação: rejeitar é governança, não ingresso.
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());
    assert_eq!(count_joins(&pool, node.identity.node_id()).await, 1);
    let (approved_by_node, signature, epoch, expires) = stored_approval(&pool, &id).await;
    assert!(
        approved_by_node.is_none() && signature.is_none() && epoch.is_none() && expires.is_none(),
        "pedido rejeitado carregando prova de aprovação: {approved_by_node:?} {signature:?}"
    );

    // Aprovar depois de rejeitar é conflito, não caminho alternativo. O
    // certificado é válido: o que recusa é o estado terminal já decidido.
    let res = approve_with(&server, &id, &cert);
    assert_eq!(res.status, 409, "aprovar rejeitado: {}", res.body);
    assert_eq!(res.json()["error"], "join_request_state_conflict");
    assert!(peer_state(&pool, node.identity.node_id()).await.is_none());

    // Rejeitar de novo também é conflito.
    let res = reject_with(&server, &id, &rejection);
    assert_eq!(res.status, 409, "rejeitar de novo: {}", res.body);

    // Pedido inexistente é 404 nas duas decisões, com certificado em mãos: a
    // recusa é de existência, não de autoridade.
    let inexistente = "fjr_00000000000000000000000000000000";
    let res = approve_with(&server, inexistente, &cert);
    assert_eq!(res.status, 404, "approve inexistente: {}", res.body);
    assert_eq!(res.json()["error"], "join_request_not_found");
    let res = reject_with(&server, inexistente, &rejection);
    assert_eq!(res.status, 404, "reject inexistente: {}", res.body);
    assert_eq!(res.json()["error"], "join_request_not_found");
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

    let approver = genesis_approver("segredo");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&approver])).await;

    let node = node_with_key("segredo");
    let seed = node.seed_hex.clone();
    let approver_seed = approver.seed_hex.clone();
    assert_eq!(seed.len(), 64);
    assert_eq!(approver_seed.len(), 64);

    let manifest = manifest_for(&node, 2, now_unix());
    let body = serde_json::to_string(&manifest).expect("manifesto serializa");
    // O próprio corpo enviado não tem a semente: só chave pública e assinatura.
    assert!(!body.contains(&seed), "pedido enviado contém a semente");

    let mut corpos = Vec::new();
    let res = join(&server, &manifest);
    assert_eq!(res.status, 201, "ingresso: {}", res.body);
    corpos.push(res.body.clone());
    let pending = res.json();
    let id = pending["join_request_id"]
        .as_str()
        .expect("id")
        .to_owned();
    let cert = fresh_certificate(&approver, &pending);

    // O corpo da decisão leva a assinatura e a chave pública do aprovador.
    let sent = decision_body(&cert);
    assert!(!sent.contains(&approver_seed), "corpo enviado contém a semente do aprovador");

    for corpo in [
        common::get(server.addr, "/v1/federation/join-requests", TOKEN_READ).body,
        common::get(server.addr, "/v1/federation/peers", TOKEN_READ).body,
        approve_with(&server, &id, &cert).body,
        common::get(server.addr, "/v1/federation/peers", TOKEN_READ).body,
    ] {
        corpos.push(corpo);
    }

    // Nenhum corpo devolve a semente de quem quer que seja; chave pública e
    // assinatura aparecem, porque são dados públicos.
    for corpo in &corpos {
        assert!(!corpo.contains(&seed), "resposta expôs a semente: {corpo}");
        assert!(
            !corpo.contains(&approver_seed),
            "resposta expôs a semente do aprovador: {corpo}"
        );
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
    assert!(
        corpos
            .iter()
            .any(|c| c.contains(&cert.signature)),
        "a prova assinada é pública e deve ser legível no pedido aprovado"
    );

    // Auditoria e log de requisição: só identificadores públicos.
    let mut registros = server.audit.lines().await;
    registros.extend(server.log.lines());
    assert!(!registros.is_empty(), "houve auditoria/log");
    for linha in &registros {
        assert!(!linha.contains(&seed), "log expôs a semente: {linha}");
        assert!(
            !linha.contains(&approver_seed),
            "log expôs a semente do aprovador: {linha}"
        );
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
    let approver = genesis_approver("estado");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&approver])).await;

    let node = node_with_key("estado");
    let pending = join_pending(&server, &node, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();
    let cert = fresh_certificate(&approver, &pending);
    let res = approve_with(&server, &id, &cert);
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

    // Aprovar de novo é idempotente e continua authorized: o mesmo certificado
    // (mesmo epoch) repete a decisão sem reescrever a evidência original.
    let res = approve_with(&server, &id, &cert);
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
    let novo_pedido = res.json();
    let id = novo_pedido["join_request_id"]
        .as_str()
        .expect("id")
        .to_owned();
    let res = approve_with(&server, &id, &fresh_certificate(&approver, &novo_pedido));
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

// --- 8) M1b: aprovação assinada por nó autorizado (emenda à ADR-071) --------

/// O caminho feliz com a segunda prova: nó genesis assina, o servidor verifica
/// contra a chave **registrada** e o par nasce `authorized` com evidência.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn genesis_aprova_com_certificado_assinado_e_a_prova_fica_registrada() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-genesis");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let candidate = node_with_key("m1b-candidato");
    let pending = join_pending(&server, &candidate, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();
    let cert = fresh_certificate(&genesis_node, &pending);

    let res = approve_with(&server, &id, &cert);
    assert_eq!(res.status, 200, "aprovação por genesis: {}", res.body);
    let body = res.json();
    assert_eq!(body["state"], "approved");
    assert_eq!(body["approved_by_node"], genesis_node.identity.node_id());
    assert_eq!(body["approval_signature"], cert.signature);
    assert_eq!(body["approval_epoch"], 1);
    assert_eq!(body["approval_expires_at"], cert.expires_at);

    // Efeito no banco, não só na resposta.
    assert_eq!(
        peer_state(&pool, candidate.identity.node_id())
            .await
            .as_deref(),
        Some("authorized"),
        "o candidato aprovado aparece como par autorizado"
    );
    let (stored_node, stored_signature, stored_epoch, stored_expires) =
        stored_approval(&pool, &id).await;
    assert_eq!(stored_node.as_deref(), Some(genesis_node.identity.node_id()));
    assert_eq!(stored_signature.as_deref(), Some(cert.signature.as_str()));
    assert_eq!(stored_epoch, Some(1));
    assert_eq!(stored_expires, Some(cert.expires_at));

    // A assinatura é verificável fora do processo: com a chave pública do
    // genesis e os campos persistidos, qualquer nó confere a decisão.
    let mut reconstruido = cert.clone();
    reconstruido.signature = String::new();
    reconstruido.sign(&genesis_node.identity);
    assert_eq!(
        reconstruido.signature, cert.signature,
        "bytes canônicos instáveis: a mesma decisão não seria re-verificável"
    );
    reconstruido
        .verify_with_key(&genesis_node.identity.public_key_hex())
        .expect("prova re-verificada por um terceiro");
}

/// Sem o certificado, o escopo humano sozinho não decide ingresso — que é
/// exatamente o que a emenda proíbe. E nada muda de estado.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn aprovar_sem_certificado_e_recusado_sem_transicao() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-sem-cert");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let candidate = node_with_key("m1b-sem-cert-cand");
    let pending = join_pending(&server, &candidate, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();

    // (a) nenhum corpo
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 422, "sem certificado: {}", res.body);
    assert_eq!(res.json()["error"], "invalid_request");
    assert!(
        res.body.contains("approval"),
        "a resposta diz o que falta: {}",
        res.body
    );

    // (b) corpo sem o campo
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/approve"),
        Some(TOKEN_ADMIN),
        Some("{}"),
    );
    assert_eq!(res.status, 422, "corpo oco: {}", res.body);
    assert_eq!(res.json()["error"], "invalid_request");

    // (c) rejeitar tem o mesmo portão
    let res = common::call(
        server.addr,
        "POST",
        &format!("/v1/federation/join-requests/{id}/reject"),
        Some(TOKEN_ADMIN),
        None,
    );
    assert_eq!(res.status, 422, "rejeição sem certificado: {}", res.body);

    // Estado intacto: continua pending, sem par, sem prova escrita.
    let res = common::get(
        server.addr,
        "/v1/federation/join-requests?state=pending",
        TOKEN_READ,
    );
    assert!(
        res.json()
            .as_array()
            .expect("lista")
            .iter()
            .any(|r| r["join_request_id"] == id.as_str()),
        "o pedido continua pending: {}",
        res.body
    );
    assert!(peer_state(&pool, candidate.identity.node_id()).await.is_none());
    let (node_col, sig_col, epoch_col, expires_col) = stored_approval(&pool, &id).await;
    assert!(
        node_col.is_none() && sig_col.is_none() && epoch_col.is_none() && expires_col.is_none(),
        "recusa escreveu prova de aprovação"
    );
}

/// Assinatura de chave que ninguém autorizou não vale nada — e não pode nem
/// mesmo "quase" aprovar.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn certificado_de_chave_nao_autorizada_recusa_com_422_e_estado_intacto() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-naoautorizada");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let candidate = node_with_key("m1b-naoautorizada-cand");
    let pending = join_pending(&server, &candidate, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();

    // Nó real, chave válida, mas não autorizada em lugar nenhum.
    let estranho = node_with_key("m1bestranho");
    let cert = fresh_certificate(&estranho, &pending);
    cert.verify().expect("a assinatura em si confere");

    let res = approve_with(&server, &id, &cert);
    assert_eq!(res.status, 422, "aprovador desconhecido: {}", res.body);
    assert_eq!(res.json()["error"], "approver_not_authorized");

    // Estado intacto.
    assert!(peer_state(&pool, candidate.identity.node_id()).await.is_none());
    let (node_col, _, _, _) = stored_approval(&pool, &id).await;
    assert_eq!(node_col, None, "recusa escreveu o aprovador");

    // Adulterar a assinatura de um certificado legítimo é outro código, e o
    // efeito é o mesmo: nada mudou.
    let mut adulterado = fresh_certificate(&genesis_node, &pending);
    let legitimo = adulterado.canonical_bytes();
    adulterado.signature = {
        let mut sig = hex::decode(&adulterado.signature).expect("assinatura em hex");
        sig[0] ^= 0x01;
        hex::encode(sig)
    };
    let res = approve_with(&server, &id, &adulterado);
    assert_eq!(res.status, 422, "assinatura adulterada: {}", res.body);
    assert_eq!(res.json()["error"], "approval_signature_invalid");
    assert_eq!(
        legitimo,
        adulterado.canonical_bytes(),
        "trocar só a assinatura não muda os bytes cobertos por ela"
    );
    assert!(peer_state(&pool, candidate.identity.node_id()).await.is_none());

    // Chave declarada diferente da registrada é identidade emprestada: o
    // certificado assinado pelo genesis, apresentando a chave do estranho, não
    // passa — nem com a assinatura refeita, porque o laço node_id↔chave quebra.
    let mut impostor = fresh_certificate(&genesis_node, &pending);
    impostor.node_id = estranho.identity.node_id().to_owned();
    impostor.public_key = estranho.identity.public_key_hex();
    impostor.signature = genesis_node.identity.sign_hex(&impostor.canonical_bytes());
    let res = approve_with(&server, &id, &impostor);
    assert_eq!(res.status, 422, "impostura de identidade: {}", res.body);
    assert!(
        matches!(
            res.json()["error"].as_str(),
            Some("approver_not_authorized") | Some("approval_signature_invalid")
        ),
        "impostura devolve código de certificado/não autorizado: {}",
        res.body
    );
    assert!(peer_state(&pool, candidate.identity.node_id()).await.is_none());
}

/// Replay de certificado vencido: 422 `approval_expired`, estado intacto.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn certificado_expirado_e_recusado_sem_transicao() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-expirado");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let candidate = node_with_key("m1b-expirado-cand");
    let pending = join_pending(&server, &candidate, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();

    // Decisão de ontem, assinada direito.
    let ontem = certificate_for(
        &genesis_node,
        &pending,
        "cluster-br-sul",
        now_unix() - 7_200,
        now_unix() - 3_600,
        1,
    );
    ontem
        .verify_with_key(&genesis_node.identity.public_key_hex())
        .expect("a assinatura em si continua válida: o que vence é a janela");

    let res = approve_with(&server, &id, &ontem);
    assert_eq!(res.status, 422, "certificado vencido: {}", res.body);
    assert_eq!(res.json()["error"], "approval_expired");
    // O corpo nomeia a janela: quem recebe sabe que foi a validade, não a
    // assinatura, o que recusou a decisão.
    assert_eq!(res.json()["expires_at"], ontem.expires_at);
    assert!(res.json()["now"].as_i64().expect("now no corpo") > ontem.expires_at);
    assert!(peer_state(&pool, candidate.identity.node_id()).await.is_none());
    let (node_col, _, _, _) = stored_approval(&pool, &id).await;
    assert_eq!(node_col, None);

    // Ainda no futuro: outro código, mesmo efeito.
    let amanha = certificate_for(
        &genesis_node,
        &pending,
        "cluster-br-sul",
        now_unix() + 3_600,
        now_unix() + 7_200,
        1,
    );
    let res = approve_with(&server, &id, &amanha);
    assert_eq!(res.status, 422, "certificado adiantado: {}", res.body);
    assert_eq!(res.json()["error"], "approval_not_yet_valid");

    // E o mesmo pedido, com certificado dentro da janela, aprova: a recusa era
    // da janela, não do pedido.
    let res = approve_with(&server, &id, &fresh_certificate(&genesis_node, &pending));
    assert_eq!(res.status, 200, "dentro da janela: {}", res.body);
    assert_eq!(
        peer_state(&pool, candidate.identity.node_id())
            .await
            .as_deref(),
        Some("authorized")
    );
}

/// Certificado assinado para `fjr_A` não decide `fjr_B`: o identificador do
/// pedido está nos bytes assinados precisamente para isso.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn certificado_aponta_para_outro_pedido_e_recusado() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-alvo");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let primeiro_cand = node_with_key("m1b-alvo-a");
    let segundo_cand = node_with_key("m1b-alvo-b");
    let primeiro = join_pending(&server, &primeiro_cand, 1);
    let segundo = join_pending(&server, &segundo_cand, 1);
    let segundo_id = segundo["join_request_id"].as_str().expect("id").to_owned();

    // Certificado legítimo, assinado pelo genesis, mas sobre o PRIMEIRO pedido.
    let cert = fresh_certificate(&genesis_node, &primeiro);
    let res = approve_with(&server, &segundo_id, &cert);
    assert_eq!(res.status, 422, "certificado de outro pedido: {}", res.body);
    assert_eq!(res.json()["error"], "approval_signature_invalid");
    assert_eq!(
        res.json()["state"],
        serde_json::Value::Null,
        "recusa de certificado não é conflito de estado"
    );
    assert!(peer_state(&pool, segundo_cand.identity.node_id()).await.is_none());
    let (node_col, _, _, _) = stored_approval(&pool, &segundo_id).await;
    assert_eq!(node_col, None);

    // O alvo certo aprova.
    let res = approve_with(&server, &segundo_id, &fresh_certificate(&genesis_node, &segundo));
    assert_eq!(res.status, 200, "certificado do pedido certo: {}", res.body);
}

/// Cluster errado: o nó aprovado é do `cluster-br-sul`, o certificado declara
// outro cluster — a autoridade é por cluster (ADR-071 emenda §1).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn certificado_de_cluster_divergente_nao_autoriza() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-cluster");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let candidate = node_with_key("m1b-cluster-cand");
    let pending = join_pending(&server, &candidate, 1);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();

    let cert = certificate_for(
        &genesis_node,
        &pending,
        "cluster-para-outro-lugar",
        now_unix() - 1,
        now_unix() + 3_600,
        1,
    );
    let res = approve_with(&server, &id, &cert);
    assert_eq!(res.status, 422, "cluster divergente: {}", res.body);
    assert_eq!(res.json()["error"], "approver_not_authorized");
    assert!(peer_state(&pool, candidate.identity.node_id()).await.is_none());
}

/// Epoch regressivo: decisão nova não é reescrita por certificado de geração
/// mais velha do mesmo nó (ADR-071 emenda §4, anti-rollback).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn epoch_de_certificado_nao_pode_regredir() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-epoch");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let primeiro_cand = node_with_key("m1b-epoch-a");
    let segundo_cand = node_with_key("m1b-epoch-b");
    let primeiro = join_pending(&server, &primeiro_cand, 1);
    let segundo = join_pending(&server, &segundo_cand, 1);
    let segundo_id = segundo["join_request_id"].as_str().expect("id").to_owned();

    // Primeira decisão com epoch alto (geração atual do aprovador).
    let alto = certificate_for(
        &genesis_node,
        &primeiro,
        "cluster-br-sul",
        now_unix() - 1,
        now_unix() + 3_600,
        7,
    );
    let id_a = primeiro["join_request_id"].as_str().expect("id").to_owned();
    let res = approve_with(&server, &id_a, &alto);
    assert_eq!(res.status, 200, "epoch 7: {}", res.body);

    // Décima segunda decisão? Não: mesmo nó, geração mais velha.
    let baixo = certificate_for(
        &genesis_node,
        &segundo,
        "cluster-br-sul",
        now_unix() - 1,
        now_unix() + 3_600,
        5,
    );
    let res = approve_with(&server, &segundo_id, &baixo);
    assert_eq!(res.status, 422, "epoch regressivo: {}", res.body);
    assert_eq!(res.json()["error"], "approval_epoch_regressed");
    assert_eq!(res.json()["recorded"], 7);
    assert_eq!(res.json()["presented"], 5);
    assert!(peer_state(&pool, segundo_cand.identity.node_id()).await.is_none());

    // Epoch igual ao registrado vale (repetir não é regredir) e maior também.
    let igual = certificate_for(
        &genesis_node,
        &segundo,
        "cluster-br-sul",
        now_unix() - 1,
        now_unix() + 3_600,
        7,
    );
    let res = approve_with(&server, &segundo_id, &igual);
    assert_eq!(res.status, 200, "epoch igual: {}", res.body);
    let (_, _, stored_epoch, _) = stored_approval(&pool, &segundo_id).await;
    assert_eq!(stored_epoch, Some(7));
}

/// Bootstrap é condição, não default: sem genesis e sem nenhum par autorizado,
/// aprovar é impossível, e a resposta diz o que falta.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sem_genesis_e_sem_par_autorizado_aprovar_e_impossivel() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let cluster = format!("cluster-bootstrap-{}", std::process::id());
    let governance = format!("single_approver:{cluster}");
    let server = serve_with_governance(pool.clone(), Genesis::empty(), Some(&governance)).await;
    let candidate = node_with_key("m1b-bootstrap");
    let pending = join_pending_in(&server, &candidate, 1, &cluster);
    let id = pending["join_request_id"].as_str().expect("id").to_owned();

    // Alguém assina um certificado perfeitamente formado, de uma chave que não
    // foi autorizada em lugar nenhum.
    let alguem = node_with_key("m1b-bootstrap-alguem");
    let cert = certificate_for(
        &alguem,
        &pending,
        &cluster,
        now_unix() - 1,
        now_unix() + 3_600,
        1,
    );
    cert.verify().expect("bem assinado");

    let res = approve_with(&server, &id, &cert);
    assert_eq!(res.status, 422, "cluster sem aprovador: {}", res.body);
    assert_eq!(res.json()["error"], "no_authorized_approver");
    assert!(
        res.body.contains("sem genesis declarado"),
        "a resposta informa que falta autoridade declarada: {}",
        res.body
    );
    assert!(peer_state(&pool, candidate.identity.node_id()).await.is_none());

    // Rejeitar também é impossível pelo mesmo motivo.
    let mut rejection = cert.clone();
    rejection.sign_for(&alguem.identity, DecisionAction::Reject);
    let res = reject_with(&server, &id, &rejection);
    assert_eq!(res.status, 422, "rejeição sem aprovador: {}", res.body);
    assert_eq!(res.json()["error"], "no_authorized_approver");

    // O pedido continua pending, sem prova escrita: a porta não abriu.
    let (node_col, sig_col, _, _) = stored_approval(&pool, &id).await;
    assert!(node_col.is_none() && sig_col.is_none());

    // Declarado o bootstrap, a mesma decisão passa — o que faltava era
    // autoridade, não o pedido.
    let server =
        serve_with_governance(pool.clone(), genesis_of(&[&alguem]), Some(&governance)).await;
    let res = approve_with(&server, &id, &cert);
    assert_eq!(res.status, 200, "com genesis: {}", res.body);
    assert_eq!(
        peer_state(&pool, candidate.identity.node_id())
            .await
            .as_deref(),
        Some("authorized")
    );
}

/// Cadeia de confiança: um nó aprovado por genesis (par `authorized` no
/// registro) aprova outro nó — sem nenhum genesis envolvido nessa segunda
/// decisão. É o que a emenda chama de "os servidores principais autorizam
/// outros servidores".
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_autorizado_pode_aprovar_outro_no() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-cadeia");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let primeiro = node_with_key("m1b-cadeia-a");
    let segundo = node_with_key("m1b-cadeia-b");

    // 1) genesis aprova A.
    let pending_a = join_pending(&server, &primeiro, 1);
    let id_a = pending_a["join_request_id"].as_str().expect("id").to_owned();
    let res = approve_with(&server, &id_a, &fresh_certificate(&genesis_node, &pending_a));
    assert_eq!(res.status, 200, "genesis aprova A: {}", res.body);
    assert_eq!(
        peer_state(&pool, primeiro.identity.node_id())
            .await
            .as_deref(),
        Some("authorized")
    );

    // 2) agora com o servidor **sem genesis** declarado: só a cadeia vale.
    let server = serve(pool.clone()).await;
    let pending_b = join_pending(&server, &segundo, 1);
    let id_b = pending_b["join_request_id"].as_str().expect("id").to_owned();
    let res = approve_with(&server, &id_b, &fresh_certificate(&primeiro, &pending_b));
    assert_eq!(res.status, 200, "A aprovado autoriza B: {}", res.body);
    assert_eq!(
        peer_state(&pool, segundo.identity.node_id())
            .await
            .as_deref(),
        Some("authorized")
    );

    // 3) A é par autorizado: a prova fica registrada com o identificador dele,
    //    e a assinatura confere contra a chave que o banco guarda para A.
    let (node_col, sig_col, _, _) = stored_approval(&pool, &id_b).await;
    assert_eq!(node_col.as_deref(), Some(primeiro.identity.node_id()));
    assert!(sig_col.is_some(), "a prova assinada tem de ficar registrada");
    let row: (String, String) = sqlx::query_as(
        "SELECT public_key, cluster FROM federation_peer WHERE node_id = $1",
    )
    .bind(primeiro.identity.node_id())
    .fetch_one(&pool)
    .await
    .expect("chave registrada de A");
    let cert = fresh_certificate(&primeiro, &pending_b);
    assert_eq!(row.0, cert.public_key);
    let mut repetido = cert.clone();
    repetido.sign(&primeiro.identity);
    assert_eq!(
        repetido.canonical_bytes(),
        cert.canonical_bytes(),
        "forma assinada instável: a cadeia não seria re-verificável"
    );
    cert.verify_with_key(&row.0)
        .expect("terceiro nó re-verifica a cadeia sem confiar em quem entregou");
}

/// Par `revoked` no registro não aprova nada, mesmo com a assinatura correta:
/// revogação ganha de identidade válida.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn par_revogado_nao_pode_aprovar() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let genesis_node = genesis_approver("m1b-revogado");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&genesis_node])).await;

    let aprovador = node_with_key("m1b-revogado-aprovador");
    let candidato = node_with_key("m1b-revogado-candidato");

    // A entra autorizado...
    let pending_a = join_pending(&server, &aprovador, 1);
    let res = approve_with(
        &server,
        pending_a["join_request_id"].as_str().expect("id"),
        &fresh_certificate(&genesis_node, &pending_a),
    );
    assert_eq!(res.status, 200, "A autorizado: {}", res.body);

    // ...e é revogado por SQL (a rota de revogação assinada é a fatia
    // seguinte; aqui o estado no banco é o que o verificador lê).
    sqlx::query("UPDATE federation_peer SET trust_state = 'revoked' WHERE node_id = $1")
        .bind(aprovador.identity.node_id())
        .execute(&pool)
        .await
        .expect("revoga A");

    let pending_b = join_pending(&server, &candidato, 1);
    let id_b = pending_b["join_request_id"].as_str().expect("id").to_owned();
    let cert = fresh_certificate(&aprovador, &pending_b);
    cert.verify().expect("assinatura válida em si");

    let res = approve_with(&server, &id_b, &cert);
    assert_eq!(res.status, 422, "aprovador revogado: {}", res.body);
    assert_eq!(res.json()["error"], "approver_not_authorized");
    assert!(peer_state(&pool, candidato.identity.node_id()).await.is_none());

    // Genesis não salva: se o registro diz `revoked`, o registro vence a
    // configuração (senão revogar um nó genesis seria letra morta).
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&aprovador])).await;
    let res = approve_with(&server, &id_b, &cert);
    assert_eq!(
        res.status, 422,
        "genesis não reativa par revogado: {}",
        res.body
    );
    assert_eq!(res.json()["error"], "approver_not_authorized");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn governanca_recusa_decisao_de_outro_cluster() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let approver = genesis_approver("governanca-cluster");
    let candidate = node_with_key("governanca-cluster-candidato");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&approver])).await;

    let cluster = "cluster-independente";
    let pending = join_pending_in(&server, &candidate, 1, cluster);
    let id = pending["join_request_id"].as_str().unwrap();
    let cert = certificate_for(
        &approver,
        &pending,
        cluster,
        now_unix(),
        now_unix() + 300,
        1,
    );
    let response = approve_with(&server, id, &cert);
    assert_eq!(response.status, 422, "{}", response.body);
    assert_eq!(response.json()["error"], "federation_governance_disabled");
    assert_eq!(stored_approval(&pool, id).await, (None, None, None, None));
    assert!(
        peer_state(&pool, candidate.identity.node_id())
            .await
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn replay_de_aprovacao_preserva_prova_e_configuracao_do_par() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let approver = genesis_approver("replay-config");
    let candidate = node_with_key("replay-config-candidato");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&approver])).await;
    let pending = join_pending(&server, &candidate, 1);
    let id = pending["join_request_id"].as_str().unwrap();
    let cert = fresh_certificate(&approver, &pending);
    let approved = approve_with(&server, id, &cert);
    assert_eq!(approved.status, 200, "{}", approved.body);

    sqlx::query("UPDATE federation_peer SET region = 'nova-regiao' WHERE node_id = $1")
        .bind(candidate.identity.node_id())
        .execute(&pool)
        .await
        .unwrap();
    let snapshot = "SELECT to_jsonb(p)::text FROM federation_peer p WHERE node_id = $1";
    let before: String = sqlx::query_scalar(snapshot)
        .bind(candidate.identity.node_id())
        .fetch_one(&pool)
        .await
        .unwrap();

    let replay = approve_with(&server, id, &cert);
    assert_eq!(replay.status, 200, "{}", replay.body);
    assert_eq!(replay.json(), approved.json());
    let after: String = sqlx::query_scalar(snapshot)
        .bind(candidate.identity.node_id())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, before, "replay preserva o registro completo do par");

    sqlx::query("UPDATE federation_peer SET trust_state = 'revoked' WHERE node_id = $1")
        .bind(candidate.identity.node_id())
        .execute(&pool)
        .await
        .unwrap();
    let refused = approve_with(&server, id, &cert);
    assert_eq!(refused.status, 409, "{}", refused.body);
    assert_eq!(
        peer_state(&pool, candidate.identity.node_id())
            .await
            .as_deref(),
        Some("revoked")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn governanca_ausente_ou_vazia_mantem_pedido_pendente() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let approver = genesis_approver("governanca-vazia");
    let candidate = node_with_key("governanca-vazia-candidato");

    let join_server = serve(pool.clone()).await;
    let pending = join_pending(&join_server, &candidate, 1);
    let id = pending["join_request_id"].as_str().unwrap();
    let cert = fresh_certificate(&approver, &pending);
    for governance in [None, Some("")] {
        let server =
            serve_with_governance(pool.clone(), genesis_of(&[&approver]), governance).await;
        let response = approve_with(&server, id, &cert);
        assert_eq!(response.status, 422, "{}", response.body);
        assert_eq!(response.json()["error"], "federation_governance_disabled");
        let mut rejection = cert.clone();
        rejection.sign_for(&approver.identity, DecisionAction::Reject);
        let response = reject_with(&server, id, &rejection);
        assert_eq!(response.status, 422, "{}", response.body);
        assert_eq!(response.json()["error"], "federation_governance_disabled");
        let state: String = sqlx::query_scalar(
            "SELECT state FROM federation_join_request WHERE join_request_id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(state, "pending");
    }
    assert!(
        peer_state(&pool, candidate.identity.node_id())
            .await
            .is_none()
    );
}

async fn waiting_backend(pool: &PgPool, blocker: i32) -> Option<i32> {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Some(pid) = sqlx::query_scalar(
                "SELECT pid FROM pg_stat_activity
                 WHERE datname = current_database() AND $1 = ANY(pg_blocking_pids(pid))
                 LIMIT 1",
            )
            .bind(blocker)
            .fetch_optional(pool)
            .await
            .unwrap()
            {
                return pid;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .ok()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn decisoes_validas_do_mesmo_genesis_aguardam_commit() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let bootstrap = genesis_approver("serial-bootstrap");
    let approver = genesis_approver("serial-aprovador");
    let server =
        Arc::new(serve_with_genesis(pool.clone(), genesis_of(&[&bootstrap, &approver])).await);
    let first = node_with_key("serial-primeiro");
    let second = node_with_key("serial-segundo");

    // O primeiro par já existe. Uma nova geração produz outro pedido válido.
    let prior = join_pending(&server, &first, 1);
    let prior_id = prior["join_request_id"].as_str().unwrap();
    let response = approve_with(&server, prior_id, &fresh_certificate(&bootstrap, &prior));
    assert_eq!(response.status, 200, "{}", response.body);
    let first_pending = join_pending(&server, &first, 2);
    let second_pending = join_pending(&server, &second, 1);
    assert!(
        peer_state(&pool, approver.identity.node_id())
            .await
            .is_none()
    );

    let mut barrier = pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *barrier)
        .await
        .unwrap();
    sqlx::query("SELECT node_id FROM federation_peer WHERE node_id = $1 FOR UPDATE")
        .bind(first.identity.node_id())
        .fetch_one(&mut *barrier)
        .await
        .unwrap();

    let decide = |pending: Value| {
        let server = Arc::clone(&server);
        let cert = fresh_certificate(&approver, &pending);
        tokio::task::spawn_blocking(move || {
            approve_with(&server, pending["join_request_id"].as_str().unwrap(), &cert)
        })
    };
    let first_response = decide(first_pending);
    let first_backend = waiting_backend(&pool, blocker)
        .await
        .expect("primeira decisão aguardando a linha do candidato");
    let second_response = decide(second_pending);
    let second_waited = waiting_backend(&pool, first_backend).await.is_some();
    barrier.commit().await.unwrap();

    let first_response = first_response.await.unwrap();
    let second_response = second_response.await.unwrap();
    assert!(second_waited, "segunda decisão deve aguardar a primeira");
    assert_eq!(first_response.status, 200, "{}", first_response.body);
    assert_eq!(second_response.status, 200, "{}", second_response.body);
    assert_eq!(first_response.json()["approval_epoch"], 1);
    assert_eq!(second_response.json()["approval_epoch"], 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn falha_de_persistencia_reverte_par_e_permite_repetir_decisao() {
    use godmesh_server::api::ApiError;
    use godmesh_server::federation::approve_join_request;

    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let approver = genesis_approver("retry-aprovador");
    let candidate = node_with_key("retry-candidato");
    let server = serve_with_genesis(pool.clone(), genesis_of(&[&approver])).await;
    let pending = join_pending(&server, &candidate, 1);
    let id = pending["join_request_id"].as_str().unwrap();
    let config = DecisionConfig::new(
        genesis_of(&[&approver]),
        Some("single_approver:cluster-br-sul"),
    )
    .unwrap();

    let cert = fresh_certificate(&approver, &pending);
    // A CHECK de approved_by é avaliada na gravação da decisão, após inserir o par.
    let failure = approve_join_request(&pool, &config, id, "", &cert, now_unix).await;
    assert!(matches!(failure, Err(ApiError::Storage(_))), "{failure:?}");
    assert!(
        peer_state(&pool, candidate.identity.node_id())
            .await
            .is_none()
    );
    assert_eq!(stored_approval(&pool, id).await, (None, None, None, None));

    let retry = approve_with(&server, id, &cert);
    assert_eq!(retry.status, 200, "{}", retry.body);
    assert_eq!(retry.json()["approval_signature"], cert.signature);
    assert_eq!(
        peer_state(&pool, candidate.identity.node_id())
            .await
            .as_deref(),
        Some("authorized")
    );
}
