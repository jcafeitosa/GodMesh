//! Diagnóstico de enlace entre nós: `GET /v1/federation/peers/{node_id}/link`.
//!
//! Este teste **não precisa de banco**: o portão de autorização usa registro em
//! memória e fonte de revogação em memória; a rota de federação não consulta
//! Postgres. O pool é `connect_lazy` de uma URL inalcançável — se o handler
//! tocasse o banco, o teste falharia, o que é o comportamento desejado.
//!
//! Prova três coisas que a fatia promete: escopo próprio, estado real vindo do
//! transporte QUIC e `unknown` quando não há evidência. O `node_id` usado é o
//! canônico de M1 (`gmn_` + `sha256` da chave), o mesmo que o banco guarda.

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use godmesh_federation::identity::node_id_for;
use godmesh_federation::link::{LinkState, PeerLinks};
use godmesh_federation::node::{FederationConfig, FederationNode};
use godmesh_federation::peer::{PeerRecord, PeerRegistry};
use godmesh_federation::NodeIdentity;
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{
    AuditSink, Authorization, CapturedAudit, Clock, FixedClock, GrantRecord, GrantRegistry,
    RevocationFuture, RevocationSource, Scope, Subject,
};
use godmesh_server::http::router_with_federation;
use sqlx::PgPool;

const TOKEN_FEDERATION: &str = "token-fed-read";
const TOKEN_SEM_ESCOPO: &str = "token-fed-sem-escopo";

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

fn record(token_id: &str, scopes: &[Scope]) -> GrantRecord {
    GrantRecord {
        token_id: token_id.to_owned(),
        subject: Subject {
            tenant_id: "tenant-fed".to_owned(),
            principal: "operador-fed".to_owned(),
        },
        scopes: scopes.to_vec(),
        not_before: 0,
        expires_at: 4_102_444_800,
        revoked_at: None,
        authority_epoch: 1,
        predicate: predicate(),
    }
}

#[derive(Debug, Default)]
struct NoRevocation;

impl RevocationSource for NoRevocation {
    fn revoked_at<'a>(&'a self, _token_id: &'a str) -> RevocationFuture<'a> {
        Box::pin(async move { Ok(None) })
    }
}

fn authorization() -> Authorization {
    let registry = GrantRegistry::from_records([
        (
            TOKEN_FEDERATION.to_owned(),
            record("grant-fed-read", &[Scope::FederationRead]),
        ),
        (
            TOKEN_SEM_ESCOPO.to_owned(),
            record("grant-fed-sem", &[Scope::IncidentRead]),
        ),
    ]);
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(1_700_000_000));
    let audit: Arc<dyn AuditSink> = Arc::new(CapturedAudit::default());
    Authorization::with_parts(registry, clock, Arc::new(NoRevocation), 5, audit)
}

/// Pool que nunca conecta: a rota de federação não toca o banco.
fn lazy_pool() -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_lazy("postgres://godmesh:godmesh_dev_only@127.0.0.1:1/godmesh")
        .expect("URL de pool lazy")
}

async fn serve(links: Arc<PeerLinks>) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = router_with_federation(lazy_pool(), authorization(), links);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("serve falhou: {e}");
        }
    });
    addr
}

fn http_get(addr: SocketAddr, path: &str, token: Option<&str>) -> (u16, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    let mut request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");
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

fn path(node_id: &str) -> String {
    format!("/v1/federation/peers/{node_id}/link")
}

fn body(raw: &str) -> String {
    raw.split("\r\n\r\n").nth(1).unwrap_or("").to_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sem_credencial_e_401() {
    let links = Arc::new(PeerLinks::new(Duration::from_secs(1)));
    let addr = serve(links).await;
    let (status, raw) = http_get(addr, &path(&node_id_for(&[1u8; 32])), None);
    assert_eq!(status, 401);
    assert!(raw.contains("unauthenticated"), "{raw}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grant_sem_escopo_de_federacao_e_403() {
    let links = Arc::new(PeerLinks::new(Duration::from_secs(1)));
    let addr = serve(links).await;
    let (status, raw) = http_get(
        addr,
        &path(&node_id_for(&[1u8; 32])),
        Some(TOKEN_SEM_ESCOPO),
    );
    assert_eq!(status, 403);
    assert!(raw.contains("insufficient_scope"), "{raw}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn par_nunca_observado_e_unknown_com_campos_nulos() {
    let links = Arc::new(PeerLinks::new(Duration::from_secs(1)));
    let addr = serve(links).await;
    let node = node_id_for(&[2u8; 32]);
    let (status, raw) = http_get(addr, &path(&node), Some(TOKEN_FEDERATION));
    assert_eq!(status, 200);
    let body = body(&raw);
    assert!(body.contains("\"state\":\"unknown\""), "{body}");
    assert!(body.contains("\"last_seen_at\":null"), "{body}");
    assert!(body.contains("\"rtt_ms\":null"), "{body}");
    assert!(body.contains("\"negotiated_alpn\":null"), "{body}");
    assert!(body.contains("\"peer_version\":null"), "{body}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn node_id_nao_canonico_e_recusado() {
    let links = Arc::new(PeerLinks::new(Duration::from_secs(1)));
    let addr = serve(links).await;
    for invalido in [
        "NAO-E-UM-NODE-ID",
        // `gmn_` com hex maiúsculo não é o formato que o banco guarda.
        "gmn_0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
        // 64 hex sem o prefixo: hex puro é o identificador de outra fatia, não o
        // canônico da federação.
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    ] {
        let (status, raw) = http_get(addr, &path(invalido), Some(TOKEN_FEDERATION));
        assert_eq!(status, 422, "{invalido}: {raw}");
        assert!(raw.contains("invalid_request"), "{invalido}: {raw}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enlace_real_aparece_como_ready() {
    // Dois nós QUIC reais no loopback; o endpoint HTTP lê o mesmo `PeerLinks`.
    let identity_a = NodeIdentity::generate_in_memory().expect("identidade A");
    let identity_b = NodeIdentity::generate_in_memory().expect("identidade B");
    let a_id = identity_a.node_id().to_owned();
    let b_id = identity_b.node_id().to_owned();

    let registry_a = Arc::new(PeerRegistry::new());
    let registry_b = Arc::new(PeerRegistry::new());
    registry_a.insert(PeerRecord::authorized_from_key(
        &identity_b.public_key_bytes(),
        "br-south",
    ));
    registry_b.insert(PeerRecord::authorized_from_key(
        &identity_a.public_key_bytes(),
        "br-south",
    ));

    let config = FederationConfig::new("br-south", "br-sul", "0.1.0");
    let node_a = FederationNode::bind(
        identity_a,
        config.clone(),
        Arc::clone(&registry_a),
        SocketAddr::from(([127, 0, 0, 1], 0)),
    )
    .expect("endpoint A");
    let node_b = FederationNode::bind(
        identity_b,
        config,
        Arc::clone(&registry_b),
        SocketAddr::from(([127, 0, 0, 1], 0)),
    )
    .expect("endpoint B");

    let addr = serve(node_a.links()).await;
    let snapshot = node_a
        .dial(node_b.local_addr().expect("endereço de B"))
        .await
        .expect("handshake aceito");
    assert_eq!(snapshot.state, LinkState::Ready);

    // A vê B; B vê A. O endpoint precisa refletir o observado, não um default.
    let (status, raw) = http_get(addr, &path(&b_id), Some(TOKEN_FEDERATION));
    assert_eq!(status, 200, "{raw}");
    let link_body = body(&raw);
    assert!(link_body.contains("\"state\":\"ready\""), "{link_body}");
    assert!(
        link_body.contains("\"negotiated_alpn\":\"godmesh/1\""),
        "{link_body}"
    );
    assert!(
        link_body.contains("\"peer_version\":\"0.1.0\""),
        "{link_body}"
    );
    assert!(link_body.contains("\"last_seen_at\":"), "{link_body}");
    assert!(!link_body.contains("\"last_seen_at\":null"), "{link_body}");
    assert!(!link_body.contains("\"rtt_ms\":null"), "{link_body}");
    // `a_id` nunca foi alvo de `dial` nesta conexão (só B foi), então o enlace
    // observado por A existe para B — e o diagnóstico de A para A não inventa.
    let (status_self, raw_self) = http_get(addr, &path(&a_id), Some(TOKEN_FEDERATION));
    assert_eq!(status_self, 200);
    assert!(
        body(&raw_self).contains("\"state\":\"unknown\""),
        "sem evidência de si mesmo: {}",
        body(&raw_self)
    );

    node_a.shutdown();
    node_b.shutdown();
}