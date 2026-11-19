//! Testes de fio do transporte entre **dois nós reais** no mesmo host (portas
//! distintas, UDP loopback).
//!
//! O tipo de manifesto usado aqui é o canônico de M1 (`SignedManifest` com
//! `node_id` `gmn_…`), não um segundo formato de fio: o transporte assina e
//! verifica exatamente o que o ingresso por API assina e verifica.
//!
//! O que cada teste prova:
//!
//! | Teste | Prova |
//! |---|---|
//! | `handshake_feliz` | mTLS nos dois sentidos, ALPN `godmesh/1`, manifesto assinado aceito, enlace `ready` com RTT e versão do par |
//! | `chave_nao_registrada` | allowlist no TLS: chave fora do registro não abre conexão |
//! | `assinatura_invalida` | manifesto com assinatura corrompida fecha com `0x474D0002` |
//! | `chave_do_manifesto_difere_do_tls` | nó autorizado não consegue se passar por outro (`0x474D0004`) |
//! | `node_id_que_nao_deriva_da_chave` | `node_id` `gmn_` válido mas de outra chave é recusado (`0x474D0003`) |
//! | `peer_pending` | nó `pending` é recusado com `0x474D0006` |
//! | `issued_at_fora_da_janela` | replay de manifesto antigo recusado com `0x474D0009` |
//! | `heartbeat_para` | silêncio por N intervalos marca `unknown`, nunca `ready` |
//! | `revogado_nao_reconecta` | nó `revoked` recusado com `0x474D0007` a cada tentativa |

use std::net::SocketAddr;
use std::num::{NonZeroU32, NonZeroUsize};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::{future::Future, pin::Pin, sync::Mutex};

use godmesh_federation::handshake::{self, Rejection};
use godmesh_federation::identity::{PUBLIC_KEY_LEN, node_id_for};
use godmesh_federation::link::{LinkSnapshot, LinkState, PeerLinks};
use godmesh_federation::node::{
    FederationConfig, FederationNode, LinkAuthorizationError, LinkAuthorizer, LinkIdentity,
    LinkPermit, OperationalSession, OperationalSessionHandler, SessionError, SessionId,
    SessionLimits,
};
use godmesh_federation::peer::{PeerRecord, PeerRegistry, TrustState};
use godmesh_federation::{NodeIdentity, SignedManifest, now_unix, transport};
use quinn::{Connection, ConnectionError, Endpoint};
use tokio::time::sleep;

const CLUSTER: &str = "br-south";
const REGION: &str = "br-sul";
const VERSION: &str = "0.1.0";

/// Nó de federação com registro próprio (a lista de pares é por nó).
struct Node {
    node: FederationNode,
    registry: Arc<PeerRegistry>,
    id: String,
    key: [u8; PUBLIC_KEY_LEN],
}

fn config() -> FederationConfig {
    FederationConfig::new(CLUSTER, REGION, VERSION)
}

fn node(config: FederationConfig) -> Node {
    node_with_authorizer(config, None)
}

fn node_with_authorizer(
    config: FederationConfig,
    authorizer: Option<Arc<dyn LinkAuthorizer>>,
) -> Node {
    let identity = NodeIdentity::generate_in_memory().expect("identidade");
    let id = identity.node_id().to_owned();
    let key = identity.public_key_bytes();
    let registry = Arc::new(PeerRegistry::new());
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let node = match authorizer {
        Some(authorizer) => FederationNode::bind_with_authorizer(
            Arc::new(identity),
            config,
            Arc::clone(&registry),
            addr,
            Some(authorizer),
        ),
        None => FederationNode::bind(identity, config, Arc::clone(&registry), addr),
    }
    .expect("endpoint");
    Node {
        node,
        registry,
        id,
        key,
    }
}

impl Node {
    fn addr(&self) -> SocketAddr {
        self.node.local_addr().expect("endereço local")
    }

    fn links(&self) -> Arc<PeerLinks> {
        self.node.links()
    }

    fn record_for(&self, state: TrustState) -> PeerRecord {
        PeerRecord::authorized_from_key(&self.key, CLUSTER).with_trust_state(state)
    }
}

/// Dois nós que se conhecem como `authorized`.
async fn pair() -> (Node, Node) {
    let a = node(config());
    let b = node(config());
    a.registry.insert(b.record_for(TrustState::Authorized));
    b.registry.insert(a.record_for(TrustState::Authorized));
    (a, b)
}

async fn wait_for_state(links: &PeerLinks, id: &str, expected: LinkState) -> LinkSnapshot {
    for _ in 0..300 {
        if let Some(snapshot) = links.snapshot(id)
            && snapshot.state == expected
        {
            return snapshot;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!(
        "enlace de {id} não chegou a {} (última foto: {:?})",
        expected.as_str(),
        links.snapshot(id)
    );
}

/// Código de fechamento de aplicação que o par informou.
async fn close_code(connection: &Connection) -> u32 {
    let _ = tokio::time::timeout(Duration::from_secs(5), connection.closed()).await;
    match connection.close_reason() {
        Some(ConnectionError::ApplicationClosed(close)) => {
            u32::try_from(u64::from(close.error_code)).expect("código de aplicação cabe em u32")
        }
        other => panic!("esperava fechamento de aplicação, veio {other:?}"),
    }
}

/// Conector "cru": só TLS + stream de handshake, para enviar manifestos
/// manipulados que a API de alto nível não permite emitir.
struct RawPeer {
    endpoint: Endpoint,
    identity: NodeIdentity,
    key: [u8; PUBLIC_KEY_LEN],
    id: String,
    registry: Arc<PeerRegistry>,
    config: FederationConfig,
}

impl RawPeer {
    fn new(config: FederationConfig) -> Self {
        let identity = NodeIdentity::generate_in_memory().expect("identidade");
        let key = identity.public_key_bytes();
        let id = identity.node_id().to_owned();
        let registry = Arc::new(PeerRegistry::new());
        let endpoint = transport::bind_client(SocketAddr::from(([127, 0, 0, 1], 0)))
            .expect("endpoint de cliente");
        Self {
            endpoint,
            identity,
            key,
            id,
            registry,
            config,
        }
    }

    fn record(&self, state: TrustState) -> PeerRecord {
        PeerRecord::authorized_from_key(&self.key, CLUSTER).with_trust_state(state)
    }

    fn registry(&self) -> Arc<PeerRegistry> {
        Arc::clone(&self.registry)
    }

    /// Confia na chave do servidor (o verificador de cliente também exige
    /// allowlist: mTLS vale nos dois sentidos).
    fn trust(&self, server: &Node) {
        self.registry
            .insert(PeerRecord::authorized_from_key(&server.key, CLUSTER));
    }

    async fn connect(&self, addr: SocketAddr) -> Connection {
        let tls = transport::NodeTls::self_issued(&self.identity).expect("certificado");
        let client = transport::client_config(Arc::clone(&self.registry), &tls).expect("config");
        self.endpoint
            .connect_with(client, addr, transport::CONNECT_SERVER_NAME)
            .expect("conexão")
            .await
            .expect("handshake TLS")
    }

    /// Envia um frame de manifesto já montado e espera o desfecho.
    async fn send_manifest_frame(&self, addr: SocketAddr, frame: Vec<u8>) -> u32 {
        let connection = self.connect(addr).await;
        let (mut send, _recv) = connection.open_bi().await.expect("stream");
        send.write_all(&frame).await.expect("escreve manifesto");
        send.finish().expect("fecha envio");
        close_code(&connection).await
    }
}

/// Manifesto canônico assinado pelo nó, com `issued_at` controlado.
fn signed(identity: &NodeIdentity, issued_at: i64) -> SignedManifest {
    let mut manifest = SignedManifest {
        node_id: String::new(),
        public_key: String::new(),
        cluster: CLUSTER.to_owned(),
        region: REGION.to_owned(),
        software_version: VERSION.to_owned(),
        alpn: transport::ALPN_V1_TEXT.to_owned(),
        capabilities: Vec::new(),
        endpoints: Vec::new(),
        epoch: 1,
        issued_at,
        signature: String::new(),
    };
    manifest.sign(identity);
    manifest
}

#[tokio::test]
async fn handshake_feliz_entre_dois_nos() {
    let (a, b) = pair().await;

    let snapshot = a.node.dial(b.addr()).await.expect("handshake aceito");
    assert_eq!(snapshot.state, LinkState::Ready, "A vê B pronto");
    assert_eq!(snapshot.negotiated_alpn.as_deref(), Some("godmesh/1"));
    assert_eq!(snapshot.peer_version.as_deref(), Some(VERSION));
    assert!(snapshot.rtt_ms.is_some(), "RTT medido no enlace vivo");
    assert!(snapshot.last_seen_at.is_some(), "heartbeat deu evidência");

    // O lado servidor também verifica o cliente: mTLS nos dois sentidos.
    let server_side = wait_for_state(&b.links(), &a.id, LinkState::Ready).await;
    assert_eq!(server_side.negotiated_alpn.as_deref(), Some("godmesh/1"));
    assert_eq!(server_side.peer_version.as_deref(), Some(VERSION));

    // E o registro de B recebeu evidência de vida de A.
    let record = b.registry.record(&a.id).expect("A registrado em B");
    assert_eq!(record.trust_state, TrustState::Authorized);
    assert!(record.last_seen_at > 0);

    a.node.shutdown();
    b.node.shutdown();
}

#[tokio::test]
async fn chave_nao_registrada_nao_abre_conexao() {
    // A allowlist age no TLS: um par cuja chave não está no registro não chega
    // nem ao stream de handshake.
    let server = node(config());
    let stranger = node(config());
    // O desconhecido até conhece o servidor, mas o servidor não o conhece.
    stranger
        .registry
        .insert(server.record_for(TrustState::Authorized));

    let error = stranger
        .node
        .dial(server.addr())
        .await
        .expect_err("servidor recusa chave não registrada no TLS");
    assert!(
        matches!(error, godmesh_federation::node::LinkError::Connect(_)),
        "recusa acontece no TLS, antes do handshake: {error:?}"
    );

    // E o registro do servidor continua sem enlace para o desconhecido.
    assert!(server.links().snapshot(&stranger.id).is_none());

    stranger.node.shutdown();
    server.node.shutdown();
}

#[tokio::test]
async fn assinatura_invalida_rejeita_com_codigo() {
    let server = node(config());
    let raw = RawPeer::new(config());
    server.registry.insert(raw.record(TrustState::Authorized));
    raw.trust(&server);

    // Corrompe a **assinatura** (não o conteúdo assinado) mantendo o frame
    // bem formado: formato e assinatura têm códigos de recusa distintos.
    let mut value = serde_json::to_value(signed(&raw.identity, now_unix())).expect("json");
    let signature = value["signature"]
        .as_str()
        .expect("assinatura hex")
        .to_owned();
    let trocado = if signature.ends_with('0') { '1' } else { '0' };
    let mut repaired = signature.clone();
    repaired.pop();
    repaired.push(trocado);
    value["signature"] = serde_json::json!(repaired);
    let frame = serde_json::to_vec(&value).expect("json");

    let code = raw.send_manifest_frame(server.addr(), frame).await;
    assert_eq!(code, Rejection::SignatureInvalid.code());
    assert_eq!(code, 0x474D_0002);

    // Nenhum enlace `ready` foi criado no servidor.
    let snapshot = server.links().snapshot(&raw.id).expect("registro");
    assert_eq!(snapshot.state, LinkState::Rejected);

    server.node.shutdown();
}

#[tokio::test]
async fn chave_do_manifesto_difere_do_certificado_tls() {
    let server = node(config());
    let raw = RawPeer::new(config());
    // O atacante (raw) e a vítima (impostada) são nós autorizados no servidor.
    let victim = NodeIdentity::generate_in_memory().expect("identidade da vítima");
    server.registry.insert(raw.record(TrustState::Authorized));
    server.registry.insert(PeerRecord::authorized_from_key(
        &victim.public_key_bytes(),
        CLUSTER,
    ));
    raw.trust(&server);

    // TLS prova posse da chave do atacante; o manifesto é assinado pela vítima.
    let forged = signed(&victim, now_unix()).to_frame().expect("frame");

    let code = raw.send_manifest_frame(server.addr(), forged).await;
    assert_eq!(code, Rejection::KeyMismatch.code());
    assert_eq!(code, 0x474D_0004);

    let observed = server.links().snapshot_or_never_seen(&raw.id);
    assert_ne!(observed.state, LinkState::Ready);
    assert_eq!(observed.last_seen_at, None);
    server.node.shutdown();
}

#[tokio::test]
async fn node_id_que_nao_deriva_da_chave_e_recusado() {
    // A ligação `node_id` `gmn_` ↔ chave é **recomputada** na recepção
    // (`sha256` da chave). Trocar o `node_id` por outro `gmn_` bem formado não
    // passa, mesmo antes de a assinatura ser conferida.
    let server = node(config());
    let raw = RawPeer::new(config());
    server.registry.insert(raw.record(TrustState::Authorized));
    raw.trust(&server);

    let other = NodeIdentity::generate_in_memory().expect("outra identidade");
    let mut forged = signed(&raw.identity, now_unix());
    forged.node_id = other.node_id().to_owned();
    let frame = forged.to_frame().expect("frame");

    let code = raw.send_manifest_frame(server.addr(), frame).await;
    assert_eq!(code, Rejection::NodeIdMismatch.code());
    assert_eq!(code, 0x474D_0003);

    server.node.shutdown();
}

#[tokio::test]
async fn peer_pending_e_recusado() {
    let server = node(config());
    let a = node(config());
    server.registry.insert(a.record_for(TrustState::Pending));
    a.registry.insert(server.record_for(TrustState::Authorized));

    let error = a
        .node
        .dial(server.addr())
        .await
        .expect_err("pending não entra");
    match error {
        godmesh_federation::node::LinkError::Rejected { code, reason } => {
            assert_eq!(code, Rejection::NotAuthorized.code());
            assert_eq!(code, 0x474D_0006);
            // Motivo sanitizado: ASCII curto, sem dado do par.
            assert_eq!(reason, "par pendente de autorizacao");
        }
        other => panic!("esperava rejeição de handshake, veio {other:?}"),
    }
    // A não promoveu o servidor a `ready` só porque a conexão TLS abriu.
    assert_ne!(
        a.links()
            .snapshot(&server.id)
            .expect("enlace registrado")
            .state,
        LinkState::Ready
    );

    a.node.shutdown();
    server.node.shutdown();
}

#[tokio::test]
async fn issued_at_fora_da_janela_e_recusado() {
    let server = node(config());
    let raw = RawPeer::new(config());
    server.registry.insert(raw.record(TrustState::Authorized));
    raw.trust(&server);

    // Uma hora atrás: replay de manifesto antigo.
    let stale = signed(&raw.identity, now_unix().saturating_sub(3_600))
        .to_frame()
        .expect("frame");

    let code = raw.send_manifest_frame(server.addr(), stale).await;
    assert_eq!(code, Rejection::StaleManifest.code());
    assert_eq!(code, 0x474D_0009);

    server.node.shutdown();
}

#[tokio::test]
async fn heartbeat_que_para_marca_unknown() {
    // Servidor real com heartbeat rápido: 100 ms × 2 = 200 ms de tolerância.
    let server = node(config().with_heartbeat(Duration::from_millis(100), 2));
    let raw = RawPeer::new(config());
    server.registry.insert(raw.record(TrustState::Authorized));
    raw.trust(&server);

    // O par cru faz o handshake e então **para de mandar heartbeat**.
    let connection = raw.connect(server.addr()).await;
    let accepted = handshake::initiate(&connection, &raw.identity, &raw.config, &raw.registry())
        .await
        .expect("handshake do par cru");
    assert_eq!(accepted.node_id, server.id);

    let ready = wait_for_state(&server.links(), &raw.id, LinkState::Ready).await;
    assert!(ready.last_seen_at.is_some());

    // Silêncio maior que 2 intervalos: `unknown`, nunca `ready`.
    sleep(Duration::from_millis(900)).await;
    let expired = server.links().snapshot(&raw.id).expect("enlace");
    assert_eq!(expired.state, LinkState::Unknown);
    assert_eq!(
        expired.rtt_ms, None,
        "medição viva não sobrevive ao silêncio"
    );
    assert!(
        expired.last_seen_at.is_some(),
        "última evidência permanece histórica"
    );

    // Mais tempo não promove nada: `unknown` é o teto sem evidência nova.
    sleep(Duration::from_millis(400)).await;
    assert_eq!(
        server.links().snapshot(&raw.id).expect("enlace").state,
        LinkState::Unknown
    );

    drop(connection);
    server.node.shutdown();
}

#[tokio::test]
async fn no_revogado_nao_reconecta() {
    let (a, b) = pair().await;
    let snapshot = a.node.dial(b.addr()).await.expect("primeira conexão");
    assert_eq!(snapshot.state, LinkState::Ready);

    // Revogação com epoch maior declarada no registro de B.
    assert!(b.registry.revoke(&a.id, 2));
    assert_eq!(
        b.registry.record(&a.id).expect("registro").trust_state,
        TrustState::Revoked
    );

    // Nova tentativa: recusada com código explícito, e de novo na seguinte —
    // revogação não "cura" com o tempo.
    for attempt in 0..2 {
        let error = a
            .node
            .dial(b.addr())
            .await
            .expect_err("revogado não reconecta");
        match error {
            godmesh_federation::node::LinkError::Rejected { code, .. } => {
                assert_eq!(code, Rejection::Revoked.code(), "tentativa {attempt}");
                assert_eq!(code, 0x474D_0007);
            }
            other => panic!("esperava rejeição por revogação, veio {other:?}"),
        }
    }

    a.node.shutdown();
    b.node.shutdown();
}

/// `node_id_for` é a única função que cria identificador; o teste fixa o formato
/// que o banco (`federation_peer.node_id`) e as rotas usam.
#[test]
fn node_id_do_transporte_e_o_mesmo_do_banco() {
    let identity = NodeIdentity::generate_in_memory().expect("identidade");
    assert_eq!(
        identity.node_id(),
        node_id_for(&identity.public_key_bytes())
    );
    assert!(identity.node_id().starts_with("gmn_"));
}
struct TestAuthorizer {
    policy: Arc<tokio::sync::RwLock<Result<Option<Instant>, LinkAuthorizationError>>>,
    changes: tokio::sync::watch::Sender<u64>,
    identities: Mutex<Vec<(LinkIdentity, LinkIdentity)>>,
}

impl TestAuthorizer {
    fn new(policy: Result<Option<Instant>, LinkAuthorizationError>) -> Arc<Self> {
        Arc::new(Self {
            policy: Arc::new(tokio::sync::RwLock::new(policy)),
            changes: tokio::sync::watch::channel(0).0,
            identities: Mutex::new(Vec::new()),
        })
    }

    async fn change(&self, policy: Result<Option<Instant>, LinkAuthorizationError>) {
        let mut current = self.policy.write().await;
        *current = policy;
        self.changes.send_modify(|version| *version += 1);
    }
}

impl LinkAuthorizer for TestAuthorizer {
    fn authorize<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        Box::pin(async move {
            self.identities
                .lock()
                .unwrap()
                .push((local.clone(), remote.clone()));
            let protection = Arc::clone(&self.policy).read_owned().await;
            let expires_at = (*protection)?;
            Ok(LinkPermit::new(Box::new(protection), expires_at))
        })
    }

    fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.subscribe()
    }
}

fn operational_pair(authorizer: &Arc<TestAuthorizer>) -> (Node, Node) {
    // Janela longa para distinguir invalidação operacional de silêncio.
    let cfg = config().with_heartbeat(Duration::from_secs(30), 3);
    let managed = node_with_authorizer(cfg.clone(), Some(authorizer.clone()));
    let legacy = node(cfg);
    managed
        .registry
        .insert(legacy.record_for(TrustState::Authorized));
    legacy
        .registry
        .insert(managed.record_for(TrustState::Authorized));
    (managed, legacy)
}

#[tokio::test]
async fn autorizador_recusa_antes_do_protocolo_nos_dois_sentidos() {
    for denial in [
        LinkAuthorizationError::Denied,
        LinkAuthorizationError::Unavailable,
    ] {
        let auth = TestAuthorizer::new(Err(denial));
        let (managed, legacy) = operational_pair(&auth);
        assert!(managed.node.dial(legacy.addr()).await.is_err());
        assert!(legacy.node.dial(managed.addr()).await.is_err());
        assert_ne!(
            managed.links().snapshot_or_never_seen(&legacy.id).state,
            LinkState::Ready
        );
        assert_eq!(
            legacy
                .links()
                .snapshot_or_never_seen(&managed.id)
                .last_seen_at,
            None,
            "nenhum manifesto foi aceito"
        );
        let expected = (
            LinkIdentity {
                node_id: managed.id.clone(),
                public_key: managed.key,
                cluster: CLUSTER.into(),
            },
            LinkIdentity {
                node_id: legacy.id.clone(),
                public_key: legacy.key,
                cluster: CLUSTER.into(),
            },
        );
        let peer = RawPeer::new(config());
        managed.registry.insert(peer.record(TrustState::Authorized));
        peer.trust(&managed);
        let connection = peer.connect(managed.addr()).await;
        // mTLS já terminou. A recusa chega mesmo sem abrir stream de manifesto.
        assert_eq!(
            close_code(&connection).await,
            Rejection::NotAuthorized.code()
        );
        let raw_identity = LinkIdentity {
            node_id: peer.id,
            public_key: peer.key,
            cluster: CLUSTER.into(),
        };
        assert_eq!(
            *auth.identities.lock().unwrap(),
            vec![
                expected.clone(),
                expected.clone(),
                (expected.0, raw_identity)
            ]
        );
        managed.node.shutdown();
        legacy.node.shutdown();
    }
}

#[tokio::test]
async fn enlace_expira_sem_esperar_proximo_heartbeat() {
    let auth = TestAuthorizer::new(Ok(Some(Instant::now() + Duration::from_secs(1))));
    let (managed, legacy) = operational_pair(&auth);
    assert_eq!(
        managed.node.dial(legacy.addr()).await.unwrap().state,
        LinkState::Ready
    );
    wait_for_state(&managed.links(), &legacy.id, LinkState::Unknown).await;
    wait_for_state(&legacy.links(), &managed.id, LinkState::Unknown).await;
    assert!(managed.node.dial(legacy.addr()).await.is_err());
    managed.node.shutdown();
    legacy.node.shutdown();
}

#[tokio::test]
async fn mudanca_fecha_enlace_e_reconexao_exige_nova_autorizacao() {
    let auth = TestAuthorizer::new(Ok(None));
    let (managed, legacy) = operational_pair(&auth);
    legacy.node.dial(managed.addr()).await.unwrap();
    wait_for_state(&managed.links(), &legacy.id, LinkState::Ready).await;

    tokio::time::timeout(
        Duration::from_secs(2),
        auth.change(Err(LinkAuthorizationError::Denied)),
    )
    .await
    .expect("a proteção foi liberada sem esperar por rede");
    wait_for_state(&managed.links(), &legacy.id, LinkState::Unknown).await;
    wait_for_state(&legacy.links(), &managed.id, LinkState::Unknown).await;
    assert!(legacy.node.dial(managed.addr()).await.is_err());

    auth.change(Ok(None)).await;
    legacy.node.dial(managed.addr()).await.unwrap();
    wait_for_state(&managed.links(), &legacy.id, LinkState::Ready).await;
    managed.node.shutdown();
    legacy.node.shutdown();
}

#[tokio::test]
async fn heartbeat_reconsulta_autoridade_sem_sinal_de_mudanca() {
    let auth = TestAuthorizer::new(Ok(None));
    let managed = node_with_authorizer(
        config().with_heartbeat(Duration::from_secs(30), 3),
        Some(auth.clone()),
    );
    let peer = RawPeer::new(config());
    managed.registry.insert(peer.record(TrustState::Authorized));
    peer.trust(&managed);
    let connection = peer.connect(managed.addr()).await;
    handshake::initiate(&connection, &peer.identity, &peer.config, &peer.registry)
        .await
        .expect("manifestos aceitos");
    wait_for_state(&managed.links(), &peer.id, LinkState::Ready).await;

    // Aguardar o heartbeat inicial garante que o próximo acesso à autoridade
    // ocorrerá por causa do heartbeat recebido.
    let mut initial = tokio::time::timeout(Duration::from_secs(2), connection.accept_uni())
        .await
        .expect("heartbeat inicial")
        .expect("stream de heartbeat");
    initial.read_to_end(12).await.expect("frame completo");

    // Uma falha de acesso à autoridade não produz alteração durável.
    *auth.policy.write().await = Err(LinkAuthorizationError::Unavailable);
    let mut heartbeat = connection.open_uni().await.unwrap();
    heartbeat
        .write_all(b"GMH1\x00\x00\x00\x00\x00\x00\x00\x01")
        .await
        .unwrap();
    heartbeat.finish().unwrap();
    assert_eq!(
        close_code(&connection).await,
        Rejection::NotAuthorized.code()
    );
    wait_for_state(&managed.links(), &peer.id, LinkState::Unknown).await;
    managed.node.shutdown();
}

#[tokio::test]
async fn reavalia_autoridade_antes_de_ready_sem_proteger_espera_pelo_manifesto() {
    let auth = TestAuthorizer::new(Ok(None));
    let managed = node_with_authorizer(config(), Some(auth.clone()));
    let peer = RawPeer::new(config());
    managed.registry.insert(peer.record(TrustState::Authorized));
    peer.trust(&managed);
    let connection = peer.connect(managed.addr()).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while auth.identities.lock().unwrap().is_empty() {
            sleep(Duration::from_millis(10)).await;
        }
        // A consulta inicial já começou e o cliente ainda não abriu stream.
        *auth.policy.write().await = Err(LinkAuthorizationError::Unavailable);
    })
    .await
    .expect("a espera pelo manifesto deve ficar fora da proteção");
    let _ = handshake::initiate(&connection, &peer.identity, &peer.config, &peer.registry).await;
    assert_eq!(
        close_code(&connection).await,
        Rejection::NotAuthorized.code()
    );
    assert_eq!(
        managed
            .links()
            .snapshot_or_never_seen(&peer.id)
            .last_seen_at,
        None
    );
    managed.node.shutdown();
}

#[tokio::test]
async fn fechamento_de_uma_conexao_preserva_outro_enlace_do_mesmo_par() {
    let auth = TestAuthorizer::new(Ok(None));
    let managed = node_with_authorizer(config(), Some(auth.clone()));
    let peer = RawPeer::new(config());
    managed.registry.insert(peer.record(TrustState::Authorized));
    peer.trust(&managed);
    let first = peer.connect(managed.addr()).await;
    handshake::initiate(&first, &peer.identity, &peer.config, &peer.registry)
        .await
        .unwrap();
    let links = managed.links();
    let first_seen = wait_for_state(&links, &peer.id, LinkState::Ready)
        .await
        .last_seen_at
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while now_unix() <= first_seen {
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("segundo handshake em outro segundo");
    let second = peer.connect(managed.addr()).await;
    let mut second_config = peer.config.clone();
    second_config.software_version = "0.2.0".into();
    handshake::initiate(&second, &peer.identity, &second_config, &peer.registry)
        .await
        .unwrap();
    // Cada stream confirma que a respectiva conexão havia alcançado ready.
    for connection in [&first, &second] {
        let mut heartbeat = tokio::time::timeout(Duration::from_secs(2), connection.accept_uni())
            .await
            .unwrap()
            .unwrap();
        heartbeat.read_to_end(12).await.unwrap();
    }
    let newest = links.snapshot_or_never_seen(&peer.id).last_seen_at.unwrap();
    assert!(newest > first_seen);
    second.close(0u32.into(), b"fim do segundo enlace");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = links.snapshot_or_never_seen(&peer.id);
            if snapshot.peer_version.as_deref() == Some(VERSION) {
                assert_eq!(snapshot.state, LinkState::Ready);
                assert_eq!(snapshot.last_seen_at, Some(newest));
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("o primeiro enlace deve voltar a representar o par");
    first.close(0u32.into(), b"fim do primeiro enlace");
    let closed = wait_for_state(&links, &peer.id, LinkState::Unknown).await;
    assert_eq!(closed.last_seen_at, Some(newest));
    assert_eq!(
        managed.registry.record(&peer.id).unwrap().last_seen_at,
        newest
    );
    managed.node.shutdown();
}

async fn send_test_heartbeat(connection: &Connection, counter: u64) {
    let mut send = connection.open_uni().await.unwrap();
    let mut frame = [0; 12];
    frame[..4].copy_from_slice(b"GMH1");
    frame[4..].copy_from_slice(&counter.to_be_bytes());
    send.write_all(&frame).await.unwrap();
    send.finish().unwrap();
}

#[tokio::test]
async fn silencio_fecha_apenas_o_enlace_operacional_afetado() {
    let auth = TestAuthorizer::new(Ok(None));
    let managed = node_with_authorizer(
        config().with_heartbeat(Duration::from_millis(100), 8),
        Some(auth),
    );
    let peer = RawPeer::new(config());
    managed.registry.insert(peer.record(TrustState::Authorized));
    peer.trust(&managed);

    let live = peer.connect(managed.addr()).await;
    handshake::initiate(&live, &peer.identity, &peer.config, &peer.registry)
        .await
        .unwrap();
    let keep_alive = tokio::spawn({
        let live = live.clone();
        async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(40));
            let mut counter = 0u64;
            loop {
                ticker.tick().await;
                counter = counter.wrapping_add(1);
                send_test_heartbeat(&live, counter).await;
            }
        }
    });

    let silent = peer.connect(managed.addr()).await;
    let mut silent_config = peer.config.clone();
    silent_config.software_version = "0.2.0".into();
    handshake::initiate(&silent, &peer.identity, &silent_config, &peer.registry)
        .await
        .unwrap();
    let mut initial = tokio::time::timeout(Duration::from_secs(2), silent.accept_uni())
        .await
        .unwrap()
        .unwrap();
    initial.read_to_end(12).await.unwrap();
    let links = managed.links();
    assert_eq!(
        links
            .snapshot_or_never_seen(&peer.id)
            .peer_version
            .as_deref(),
        Some("0.2.0")
    );

    // Ambos os enlaces estão autorizados sem lease. Apenas o segundo ficou mudo.
    assert!(
        matches!(
            tokio::time::timeout(Duration::from_secs(3), silent.closed()).await,
            Ok(ConnectionError::ApplicationClosed(_))
        ),
        "o silêncio precisa fechar a conexão QUIC"
    );
    assert!(
        silent.open_uni().await.is_err(),
        "sessão expirada não aceita mais streams"
    );
    assert!(live.close_reason().is_none());
    let snapshot = links.snapshot_or_never_seen(&peer.id);
    assert_eq!(snapshot.state, LinkState::Ready);
    assert_eq!(snapshot.peer_version.as_deref(), Some(VERSION));

    keep_alive.abort();
    live.close(0u32.into(), b"fim do enlace de controle");
    wait_for_state(&links, &peer.id, LinkState::Unknown).await;
    let fresh = peer.connect(managed.addr()).await;
    handshake::initiate(&fresh, &peer.identity, &peer.config, &peer.registry)
        .await
        .unwrap();
    wait_for_state(&links, &peer.id, LinkState::Ready).await;
    managed.node.shutdown();
}

#[tokio::test]
async fn heartbeat_atrasado_entre_varreduras_nao_renova_evidencia() {
    let interval = Duration::from_secs(3);
    let managed = node_with_authorizer(
        config().with_heartbeat(interval, 1),
        Some(TestAuthorizer::new(Ok(None))),
    );
    let peer = RawPeer::new(config());
    managed.registry.insert(peer.record(TrustState::Authorized));
    peer.trust(&managed);
    let connection = peer.connect(managed.addr()).await;
    handshake::initiate(&connection, &peer.identity, &peer.config, &peer.registry)
        .await
        .unwrap();
    let mut initial = tokio::time::timeout(Duration::from_secs(2), connection.accept_uni())
        .await
        .unwrap()
        .unwrap();
    initial.read_to_end(12).await.unwrap();

    // Use outro segundo Unix para confirmar a aplicação do heartbeat e
    // desloque seu prazo em relação ao monitor e ao emissor local.
    let before = managed.registry.record(&peer.id).unwrap().last_seen_at;
    sleep(Duration::from_millis(1150)).await;
    assert!(now_unix() > before);
    send_test_heartbeat(&connection, 1).await;
    tokio::time::timeout(Duration::from_millis(400), async {
        while managed.registry.record(&peer.id).unwrap().last_seen_at <= before {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("primeiro heartbeat aceito");
    let links = managed.links();
    let last_seen = links.snapshot_or_never_seen(&peer.id).last_seen_at.unwrap();
    assert!(last_seen > before);
    assert_eq!(
        managed.registry.record(&peer.id).unwrap().last_seen_at,
        last_seen
    );

    sleep(interval + Duration::from_millis(150)).await;
    assert!(
        connection.close_reason().is_none(),
        "antes da próxima varredura"
    );
    assert_eq!(
        links.snapshot_or_never_seen(&peer.id).state,
        LinkState::Unknown
    );
    send_test_heartbeat(&connection, 2).await;
    assert!(
        matches!(
            tokio::time::timeout(Duration::from_millis(600), connection.closed()).await,
            Ok(ConnectionError::ApplicationClosed(_))
        ),
        "o heartbeat atrasado deve encerrar a sessão"
    );
    assert_eq!(
        links.snapshot_or_never_seen(&peer.id).last_seen_at,
        Some(last_seen)
    );
    assert_eq!(
        managed.registry.record(&peer.id).unwrap().last_seen_at,
        last_seen
    );
    managed.node.shutdown();
}

fn attempt_deadline() -> tokio::time::Instant {
    tokio::time::Instant::now() + Duration::from_secs(4)
}

struct SessionSink(tokio::sync::mpsc::UnboundedSender<OperationalSession>);

impl OperationalSessionHandler for SessionSink {
    fn run(&self, session: OperationalSession) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.0.send(session).ok();
        })
    }
}

struct PromotionGate {
    calls: AtomicUsize,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    reject: bool,
    authority: Arc<TestAuthorizer>,
}

impl LinkAuthorizer for PromotionGate {
    fn authorize<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        Box::pin(async move {
            if self.calls.fetch_add(1, Ordering::Relaxed) == 1 {
                self.entered.notify_one();
                self.release.notified().await;
                if self.reject {
                    return Err(LinkAuthorizationError::Denied);
                }
            }
            self.authority.authorize(local, remote).await
        })
    }

    fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.authority.subscribe()
    }
}

fn session_node(
    peer: &RawPeer,
    authorizer: Arc<dyn LinkAuthorizer>,
) -> (
    FederationNode,
    tokio::sync::mpsc::UnboundedReceiver<OperationalSession>,
) {
    session_node_with_config(
        peer,
        authorizer,
        config().with_heartbeat(Duration::from_secs(30), 3),
    )
}

fn session_node_with_config(
    peer: &RawPeer,
    authorizer: Arc<dyn LinkAuthorizer>,
    config: FederationConfig,
) -> (
    FederationNode,
    tokio::sync::mpsc::UnboundedReceiver<OperationalSession>,
) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let node = bind_session_fixture(peer, authorizer, config, Arc::new(SessionSink(tx)));
    (node, rx)
}

fn bind_session_fixture(
    peer: &RawPeer,
    authorizer: Arc<dyn LinkAuthorizer>,
    config: FederationConfig,
    handler: Arc<dyn OperationalSessionHandler>,
) -> FederationNode {
    bind_session_fixture_with_timeout(peer, authorizer, config, handler, Duration::from_secs(4))
}

fn bind_session_fixture_with_timeout(
    peer: &RawPeer,
    authorizer: Arc<dyn LinkAuthorizer>,
    config: FederationConfig,
    handler: Arc<dyn OperationalSessionHandler>,
    attempt_timeout: Duration,
) -> FederationNode {
    let identity = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    peer.registry.insert(PeerRecord::authorized_from_key(
        &identity.public_key_bytes(),
        CLUSTER,
    ));
    let registry = Arc::new(PeerRegistry::new());
    registry.insert(peer.record(TrustState::Authorized));
    FederationNode::bind_with_session_handler(
        identity,
        config,
        registry,
        ([127, 0, 0, 1], 0).into(),
        authorizer,
        SessionLimits {
            attempt_timeout,
            max_frame_bytes: NonZeroU32::new(4).unwrap(),
            max_streams_per_connection: NonZeroUsize::new(1).unwrap(),
        },
        handler,
    )
    .unwrap()
}

async fn session_handshake(node: &FederationNode, peer: &RawPeer) -> Connection {
    let connection = peer.connect(node.local_addr().unwrap()).await;
    handshake::initiate(&connection, &peer.identity, &peer.config, &peer.registry)
        .await
        .unwrap();
    connection
}

async fn next_session(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<OperationalSession>,
) -> OperationalSession {
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn stream_enfileirada_so_chega_ao_callback_apos_promocao() {
    for reject in [false, true] {
        let peer = RawPeer::new(config());
        let gate = Arc::new(PromotionGate {
            calls: AtomicUsize::new(0),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            reject,
            authority: TestAuthorizer::new(Ok(None)),
        });
        let (node, mut callbacks) = session_node(&peer, gate.clone());
        let connection = session_handshake(&node, &peer).await;
        tokio::time::timeout(Duration::from_secs(2), gate.entered.notified())
            .await
            .unwrap();
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        send.write_all(b"\0\0\0\x04pull").await.unwrap();
        send.finish().unwrap();
        assert!(callbacks.try_recv().is_err());
        assert_ne!(
            node.links().snapshot_or_never_seen(&peer.id).state,
            LinkState::Ready
        );

        gate.release.notify_one();
        if reject {
            assert_eq!(
                close_code(&connection).await,
                Rejection::NotAuthorized.code()
            );
            assert!(callbacks.try_recv().is_err());
        } else {
            let session = next_session(&mut callbacks).await;
            assert_eq!(session.remote().public_key, peer.key);
            assert_eq!(session.local().node_id, node.identity().node_id());
            assert_eq!(session.negotiated_alpn(), "godmesh/1");
            let mut stream = session.accept_bi().await.unwrap();
            assert_eq!(stream.read_frame().await.unwrap(), b"pull");
            stream.expect_fin().await.unwrap();
            stream.write_frame(b"done").await.unwrap();
            stream.finish_send().unwrap();
            drop(stream);
            assert_eq!(recv.read_to_end(8).await.unwrap(), b"\0\0\0\x04done");
            assert_eq!(
                node.links().snapshot_or_never_seen(&peer.id).state,
                LinkState::Ready
            );
        }
        node.shutdown();
    }
}

#[tokio::test]
async fn cota_e_framing_sao_por_stream_e_preservam_fin() {
    let peer = RawPeer::new(config());
    let (node, mut callbacks) = session_node(&peer, TestAuthorizer::new(Ok(None)));
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut callbacks).await;
    let mut outgoing = session.open_bi(attempt_deadline()).await.unwrap();
    assert!(matches!(
        session.open_bi(attempt_deadline()).await,
        Err(SessionError::Busy)
    ));
    let (mut overflow_send, mut overflow_recv) = connection.open_bi().await.unwrap();
    overflow_send.write_all(b"\0\0\0\x04pull").await.unwrap();
    assert!(matches!(session.accept_bi().await, Err(SessionError::Busy)));
    assert!(overflow_recv.read_to_end(8).await.is_err());
    assert_eq!(
        outgoing.write_frame(b"large").await,
        Err(SessionError::InvalidFrame)
    );
    outgoing.write_frame(b"ping").await.unwrap();
    outgoing.finish_send().unwrap();
    let (mut reply, mut request) = connection.accept_bi().await.unwrap();
    assert_eq!(request.read_to_end(8).await.unwrap(), b"\0\0\0\x04ping");
    reply.write_all(b"\0\0\0\x04pongx").await.unwrap();
    reply.finish().unwrap();
    assert_eq!(outgoing.read_frame().await.unwrap(), b"pong");
    assert_eq!(outgoing.expect_fin().await, Err(SessionError::InvalidFrame));
    assert_eq!(outgoing.expect_fin().await, Err(SessionError::Closed));
    assert!(session.open_bi(attempt_deadline()).await.is_ok());
    drop(outgoing);

    for header in [b"\0\0\0\0".as_slice(), b"\0\0\0\x05", b"\0\0"] {
        let (mut send, _) = connection.open_bi().await.unwrap();
        send.write_all(header).await.unwrap();
        send.finish().unwrap();
        let mut inbound = session.accept_bi().await.unwrap();
        assert_eq!(inbound.read_frame().await, Err(SessionError::InvalidFrame));
    }
    node.shutdown();
}

#[tokio::test]
async fn perda_de_uma_sessao_nao_cancela_sessao_paralela() {
    let peer = RawPeer::new(config());
    let auth = TestAuthorizer::new(Ok(None));
    let (node, mut callbacks) = session_node(&peer, auth.clone());
    let first = session_handshake(&node, &peer).await;
    let old = next_session(&mut callbacks).await;
    let second = session_handshake(&node, &peer).await;
    let current = next_session(&mut callbacks).await;
    assert_ne!(old.id(), current.id());

    first.close(0u32.into(), b"fim da primeira sessao");
    tokio::time::timeout(Duration::from_secs(2), old.cancelled())
        .await
        .unwrap();
    assert!(matches!(
        old.open_bi(attempt_deadline()).await,
        Err(SessionError::Closed)
    ));
    let mut active = current.open_bi(attempt_deadline()).await.unwrap();
    assert!(second.close_reason().is_none());

    let (pending_read, ()) = tokio::join!(active.read_frame(), async {
        auth.change(Ok(None)).await;
        tokio::time::timeout(Duration::from_secs(2), current.cancelled())
            .await
            .unwrap();
    },);
    assert_eq!(pending_read, Err(SessionError::Closed));
    assert!(matches!(
        current.accept_bi().await,
        Err(SessionError::Closed)
    ));
    node.shutdown();
}

#[tokio::test]
async fn guard_negado_apos_promocao_nao_entrega_stream() {
    let peer = RawPeer::new(config());
    let auth = TestAuthorizer::new(Ok(None));
    let (node, mut callbacks) = session_node(&peer, auth.clone());
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut callbacks).await;
    let mut beat = tokio::time::timeout(Duration::from_secs(2), connection.accept_uni())
        .await
        .unwrap()
        .unwrap();
    beat.read_to_end(12).await.unwrap();
    // Simula a mudança durável sem notificação, antes do próximo heartbeat.
    *auth.policy.write().await = Err(LinkAuthorizationError::Denied);
    let (mut send, _) = connection.open_bi().await.unwrap();
    send.write_all(b"\0\0\0\x04pull").await.unwrap();
    send.finish().unwrap();
    assert!(matches!(
        session.accept_bi().await,
        Err(SessionError::Closed)
    ));
    assert_eq!(
        close_code(&connection).await,
        Rejection::NotAuthorized.code()
    );
    assert_ne!(
        node.links().snapshot_or_never_seen(&peer.id).state,
        LinkState::Ready
    );
    node.shutdown();
}

#[tokio::test]
async fn prazo_e_silencio_cancelam_io_da_sessao() {
    for lease in [true, false] {
        let peer = RawPeer::new(config());
        let auth = TestAuthorizer::new(Ok(lease.then(|| Instant::now() + Duration::from_secs(1))));
        let interval = if lease {
            Duration::from_secs(30)
        } else {
            Duration::from_millis(100)
        };
        let (node, mut callbacks) =
            session_node_with_config(&peer, auth, config().with_heartbeat(interval, 4));
        let connection = session_handshake(&node, &peer).await;
        let session = next_session(&mut callbacks).await;
        let mut stream = session.open_bi(attempt_deadline()).await.unwrap();
        stream.write_frame(b"pull").await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), stream.read_frame())
                .await
                .unwrap(),
            Err(SessionError::Closed),
        );
        session.cancelled().await;
        assert!(matches!(
            session.open_bi(attempt_deadline()).await,
            Err(SessionError::Closed)
        ));
        assert!(
            tokio::time::timeout(Duration::from_secs(2), connection.closed())
                .await
                .is_ok()
        );
        node.shutdown();
    }
}

struct CleanupObserver {
    started: tokio::sync::mpsc::UnboundedSender<SessionId>,
    cancelled: tokio::sync::mpsc::UnboundedSender<SessionId>,
    cleanup: tokio::sync::Semaphore,
    finished: tokio::sync::mpsc::UnboundedSender<SessionId>,
}

impl OperationalSessionHandler for CleanupObserver {
    fn run(&self, session: OperationalSession) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.started.send(session.id()).unwrap();
            session.cancelled().await;
            self.cancelled.send(session.id()).unwrap();
            self.cleanup.acquire().await.unwrap().forget();
            self.finished.send(session.id()).unwrap();
        })
    }
}

#[tokio::test]
async fn limpa_callback_cancelado_sem_interferir_na_conexao_substituta() {
    let peer = RawPeer::new(config());
    let auth = TestAuthorizer::new(Ok(None));
    let (started, mut start) = tokio::sync::mpsc::unbounded_channel();
    let (cancelled, mut cancel) = tokio::sync::mpsc::unbounded_channel();
    let (finished, mut finish) = tokio::sync::mpsc::unbounded_channel();
    let observer = Arc::new(CleanupObserver {
        started,
        cancelled,
        cleanup: tokio::sync::Semaphore::new(0),
        finished,
    });
    let node = bind_session_fixture(&peer, auth.clone(), config(), observer.clone());
    let next = async |events: &mut tokio::sync::mpsc::UnboundedReceiver<SessionId>| {
        tokio::time::timeout(Duration::from_secs(2), events.recv())
            .await
            .unwrap()
            .unwrap()
    };
    let _old = session_handshake(&node, &peer).await;
    let old_id = next(&mut start).await;
    auth.change(Ok(None)).await;
    assert_eq!(next(&mut cancel).await, old_id);

    let new = session_handshake(&node, &peer).await;
    let new_id = next(&mut start).await;
    assert_ne!(old_id, new_id);
    observer.cleanup.add_permits(1);
    assert_eq!(next(&mut finish).await, old_id);
    assert_eq!(
        node.links().snapshot_or_never_seen(&peer.id).state,
        LinkState::Ready
    );
    assert!(new.close_reason().is_none());
    assert!(cancel.try_recv().is_err());

    node.shutdown();
    assert_eq!(next(&mut cancel).await, new_id);
    observer.cleanup.add_permits(1);
    assert_eq!(next(&mut finish).await, new_id);
}

struct SlowStreamAuthorization {
    authority: Arc<TestAuthorizer>,
    deadlines: Mutex<Vec<tokio::time::Instant>>,
}

impl LinkAuthorizer for SlowStreamAuthorization {
    fn authorize<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        self.authority.authorize(local, remote)
    }

    fn authorize_until<'a>(
        &'a self,
        local: &'a LinkIdentity,
        remote: &'a LinkIdentity,
        deadline: tokio::time::Instant,
    ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>> {
        Box::pin(async move {
            let attempt = {
                let mut deadlines = self.deadlines.lock().unwrap();
                deadlines.push(deadline);
                deadlines.len()
            };
            match attempt {
                1 => std::future::pending::<()>().await,
                2 => return Err(LinkAuthorizationError::Timeout),
                3 => sleep(Duration::from_millis(200)).await,
                _ => {}
            }
            self.authority.authorize(local, remote).await
        })
    }

    fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
        self.authority.subscribe()
    }
}

#[tokio::test]
async fn prazo_da_autorizacao_libera_cota_e_preserva_sessao() {
    let peer = RawPeer::new(config());
    let authority = Arc::new(SlowStreamAuthorization {
        authority: TestAuthorizer::new(Ok(None)),
        deadlines: Mutex::new(Vec::new()),
    });
    let (tx, mut callbacks) = tokio::sync::mpsc::unbounded_channel();
    let node = bind_session_fixture_with_timeout(
        &peer,
        authority.clone(),
        config(),
        Arc::new(SessionSink(tx)),
        Duration::from_millis(350),
    );
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut callbacks).await;
    let (mut send, mut recv) = connection.open_bi().await.unwrap();
    send.write_all(b"\0\0\0\x04pull").await.unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), session.accept_bi()).await,
        Ok(Err(SessionError::Timeout))
    ));
    assert!(recv.read_to_end(8).await.is_err());
    assert!(connection.close_reason().is_none());
    assert_eq!(
        node.links().snapshot_or_never_seen(&peer.id).state,
        LinkState::Ready
    );

    let (mut send, mut recv) = connection.open_bi().await.unwrap();
    send.write_all(b"\0\0\0\x04pull").await.unwrap();
    assert!(matches!(
        session.accept_bi().await,
        Err(SessionError::Timeout)
    ));
    assert!(recv.read_to_end(8).await.is_err());
    assert!(connection.close_reason().is_none());

    let (mut send, _recv) = connection.open_bi().await.unwrap();
    send.write_all(b"\0\0\0\x04pull").await.unwrap();
    let mut next = session.accept_bi().await.unwrap();
    assert_eq!(next.deadline(), authority.deadlines.lock().unwrap()[2]);
    assert_eq!(next.read_frame().await.unwrap(), b"pull");
    // O peer não termina esta direção. O mesmo deadline vale para o FIN.
    assert_eq!(
        tokio::time::timeout_at(
            next.deadline() + Duration::from_millis(80),
            next.expect_fin()
        )
        .await,
        Ok(Err(SessionError::Timeout)),
    );
    assert!(connection.close_reason().is_none());

    let expired = tokio::time::Instant::now();
    assert!(matches!(
        session.open_bi(expired).await,
        Err(SessionError::Timeout)
    ));
    let start = tokio::time::Instant::now();
    let deadline = start + Duration::from_secs(8);
    let outgoing = session.open_bi(deadline).await.unwrap();
    assert_eq!(outgoing.deadline(), deadline);
    let guard_deadline = authority.deadlines.lock().unwrap()[3];
    assert!(guard_deadline >= start + Duration::from_secs(5));
    assert!(guard_deadline <= tokio::time::Instant::now() + Duration::from_secs(5));
    assert!(guard_deadline < deadline);
    node.shutdown();
}

#[cfg(feature = "snapshot-harness")]
mod snapshot_transport {
    use super::*;
    use godmesh_federation::node::{SnapshotSessionLimits, StreamOpening};

    fn size(value: u32) -> NonZeroU32 {
        NonZeroU32::new(value).unwrap()
    }

    fn fixture(
        peer: &RawPeer,
        auth: Arc<dyn LinkAuthorizer>,
        opt_in: bool,
        timeout: Duration,
    ) -> (
        FederationNode,
        tokio::sync::mpsc::UnboundedReceiver<OperationalSession>,
    ) {
        fixture_with_streams(peer, auth, opt_in, timeout, 1)
    }

    fn fixture_with_streams(
        peer: &RawPeer,
        auth: Arc<dyn LinkAuthorizer>,
        opt_in: bool,
        timeout: Duration,
        streams: usize,
    ) -> (
        FederationNode,
        tokio::sync::mpsc::UnboundedReceiver<OperationalSession>,
    ) {
        let identity = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        peer.registry.insert(PeerRecord::authorized_from_key(
            &identity.public_key_bytes(),
            CLUSTER,
        ));
        let registry = Arc::new(PeerRegistry::new());
        registry.insert(peer.record(TrustState::Authorized));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handler = Arc::new(SessionSink(tx));
        let config = config().with_heartbeat(Duration::from_secs(30), 3);
        let a2 = SessionLimits {
            max_frame_bytes: size(4),
            max_streams_per_connection: NonZeroUsize::new(streams).unwrap(),
            attempt_timeout: timeout,
        };
        let addr = ([127, 0, 0, 1], 0).into();
        let node = if opt_in {
            FederationNode::bind_with_snapshot_session_handler(
                identity,
                config,
                registry,
                addr,
                auth,
                SnapshotSessionLimits {
                    a2,
                    max_session_frame_bytes: size(16),
                    max_control_bytes: size(6),
                    max_snapshot_bytes: size(20),
                },
                handler,
            )
        } else {
            FederationNode::bind_with_session_handler(
                identity, config, registry, addr, auth, a2, handler,
            )
        }
        .unwrap();
        (node, rx)
    }

    async fn send_raw(
        connection: &Connection,
        bytes: &[u8],
    ) -> (quinn::SendStream, quinn::RecvStream) {
        let (mut send, recv) = connection.open_bi().await.unwrap();
        send.write_all(bytes).await.unwrap();
        (send, recv)
    }

    #[tokio::test]
    async fn teto_a2_e_escape_dependem_apenas_do_perfil_da_abertura() {
        for opt_in in [false, true] {
            let peer = RawPeer::new(config());
            let (node, mut callbacks) = fixture(
                &peer,
                TestAuthorizer::new(Ok(None)),
                opt_in,
                Duration::from_secs(3),
            );
            let connection = session_handshake(&node, &peer).await;
            let session = next_session(&mut callbacks).await;
            let (mut send, _recv) = send_raw(&connection, b"\0\0\0\x04pull").await;
            send.finish().unwrap();
            let mut stream = session.accept_bi().await.unwrap();
            assert_eq!(
                stream.read_opening().await.unwrap(),
                StreamOpening::A2(b"pull".to_vec())
            );
            stream.expect_fin().await.unwrap();
            assert_eq!(
                stream.write_frame(b"12345").await,
                Err(SessionError::InvalidFrame)
            );
            drop(stream);

            let (holding, _recv) = send_raw(&connection, &5u32.to_be_bytes()).await;
            let mut stream = session.accept_bi().await.unwrap();
            assert_eq!(
                tokio::time::timeout(Duration::from_millis(500), stream.read_opening()).await,
                Ok(Err(SessionError::InvalidFrame)),
            );
            drop(holding);
            drop(stream);

            let prefix = if opt_in {
                b"\0\0\0\0\0\0\0\x07".as_slice()
            } else {
                b"\0\0\0\0"
            };
            let (_holding, _recv) = send_raw(&connection, prefix).await;
            let mut stream = session.accept_bi().await.unwrap();
            // Nenhum corpo foi enviado. O teto tem de ser aplicado no prefixo.
            assert_eq!(
                tokio::time::timeout(Duration::from_millis(500), stream.read_opening()).await,
                Ok(Err(SessionError::InvalidFrame)),
            );
            node.shutdown();
        }
    }

    #[tokio::test]
    async fn pacote_binario_exato_e_fin_apos_controle() {
        let peer = RawPeer::new(config());
        let (node, mut callbacks) = fixture(
            &peer,
            TestAuthorizer::new(Ok(None)),
            true,
            Duration::from_secs(3),
        );
        let connection = session_handshake(&node, &peer).await;
        let session = next_session(&mut callbacks).await;
        let deadline = attempt_deadline();
        let mut outgoing = session.open_bi(deadline).await.unwrap();
        assert_eq!(
            outgoing.write_snapshot_opening(b"1234567").await,
            Err(SessionError::InvalidFrame)
        );
        outgoing.write_snapshot_opening(b"123456").await.unwrap();
        let (mut send, mut recv) = connection.accept_bi().await.unwrap();
        let mut opening = [0; 14];
        recv.read_exact(&mut opening).await.unwrap();
        assert_eq!(&opening, b"\0\0\0\0\0\0\0\x06123456");

        send.write_all(b"\0\0\0\x06header\0\0\0\x08 \0\xffframe")
            .await
            .unwrap();
        send.finish().unwrap();
        assert_eq!(outgoing.read_snapshot_control().await.unwrap(), b"header");
        assert_eq!(
            outgoing.read_snapshot_body(size(8), size(8)).await.unwrap(),
            b" \0\xffframe"
        );
        outgoing.expect_fin().await.unwrap();
        outgoing.write_snapshot_control(b"ack").await.unwrap();
        outgoing.finish_send().unwrap();
        assert_eq!(recv.read_to_end(7).await.unwrap(), b"\0\0\0\x03ack");
        drop(outgoing);

        let (mut send, mut recv) = send_raw(&connection, b"\0\0\0\0\0\0\0\x03req").await;
        send.finish().unwrap();
        let mut inbound = session.accept_bi().await.unwrap();
        assert_eq!(
            inbound.read_opening().await.unwrap(),
            StreamOpening::Snapshot(b"req".to_vec())
        );
        inbound.expect_fin().await.unwrap();
        assert_eq!(
            inbound
                .write_snapshot_body(&[0; 17], size(17), size(20))
                .await,
            Err(SessionError::InvalidFrame)
        );
        assert_eq!(
            inbound.write_snapshot_body(b"1234", size(5), size(8)).await,
            Err(SessionError::InvalidFrame)
        );
        inbound.write_snapshot_control(b"header").await.unwrap();
        inbound
            .write_snapshot_body(&[7; 16], size(16), size(16))
            .await
            .unwrap();
        inbound.finish_send().unwrap();
        let expected = [b"\0\0\0\x06header\0\0\0\x10".as_slice(), &[7; 16]].concat();
        assert_eq!(recv.read_to_end(expected.len()).await.unwrap(), expected);
        node.shutdown();
    }

    #[tokio::test]
    async fn pacote_incompativel_recusa_so_o_prefixo_e_respeita_deadline() {
        let peer = RawPeer::new(config());
        let (node, mut callbacks) = fixture(
            &peer,
            TestAuthorizer::new(Ok(None)),
            true,
            Duration::from_secs(3),
        );
        let connection = session_handshake(&node, &peer).await;
        let session = next_session(&mut callbacks).await;
        for (prefix, expected, negotiated) in [(7, 8, 9), (8, 8, 7), (17, 17, 20)] {
            let deadline = attempt_deadline();
            let mut outgoing = session.open_bi(deadline).await.unwrap();
            outgoing.write_snapshot_opening(b"req").await.unwrap();
            let (mut send, _recv) = connection.accept_bi().await.unwrap();
            send.write_all(&size(prefix).get().to_be_bytes())
                .await
                .unwrap();
            assert_eq!(
                tokio::time::timeout(
                    Duration::from_millis(500),
                    outgoing.read_snapshot_body(size(expected), size(negotiated))
                )
                .await,
                Ok(Err(SessionError::InvalidFrame)),
            );
        }

        let deadline = tokio::time::Instant::now() + Duration::from_millis(200);
        let mut outgoing = session.open_bi(deadline).await.unwrap();
        outgoing.write_snapshot_opening(b"req").await.unwrap();
        let (mut send, _recv) = connection.accept_bi().await.unwrap();
        send.write_all(&8u32.to_be_bytes()).await.unwrap();
        assert_eq!(outgoing.deadline(), deadline);
        assert_eq!(
            outgoing.read_snapshot_body(size(8), size(8)).await,
            Err(SessionError::Timeout)
        );
        assert!(session.open_bi(attempt_deadline()).await.is_ok());
        node.shutdown();
    }

    #[tokio::test]
    async fn corpo_snapshot_exige_comprimento_e_fim_de_direcao() {
        let peer = RawPeer::new(config());
        let (node, mut callbacks) = fixture(
            &peer,
            TestAuthorizer::new(Ok(None)),
            true,
            Duration::from_secs(3),
        );
        let connection = session_handshake(&node, &peer).await;
        let session = next_session(&mut callbacks).await;

        for (bytes, finish, expected) in [
            (
                b"\0\0\0\x04abc".as_slice(),
                true,
                SessionError::InvalidFrame,
            ),
            (
                b"\0\0\0\x04abcdx".as_slice(),
                true,
                SessionError::InvalidFrame,
            ),
            (b"\0\0\0\x04abcd".as_slice(), false, SessionError::Timeout),
        ] {
            let deadline = tokio::time::Instant::now() + Duration::from_millis(250);
            let mut outgoing = session.open_bi(deadline).await.unwrap();
            outgoing.write_snapshot_opening(b"req").await.unwrap();
            let (mut send, _recv) = connection.accept_bi().await.unwrap();
            send.write_all(bytes).await.unwrap();
            if finish {
                send.finish().unwrap();
            }
            let result = async {
                assert_eq!(
                    outgoing.read_snapshot_body(size(4), size(4)).await?,
                    b"abcd"
                );
                outgoing.expect_fin().await
            }
            .await;
            assert_eq!(result, Err(expected));
            assert!(connection.close_reason().is_none());
        }
        node.shutdown();
    }

    #[tokio::test]
    async fn escape_enfileirado_nao_chega_ao_callback_antes_da_promocao() {
        let peer = RawPeer::new(config());
        let gate = Arc::new(PromotionGate {
            calls: AtomicUsize::new(0),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            reject: false,
            authority: TestAuthorizer::new(Ok(None)),
        });
        let (node, mut callbacks) = fixture(&peer, gate.clone(), true, Duration::from_secs(3));
        let connection = session_handshake(&node, &peer).await;
        tokio::time::timeout(Duration::from_secs(2), gate.entered.notified())
            .await
            .unwrap();
        let (mut send, _recv) = send_raw(&connection, b"\0\0\0\0\0\0\0\x03req").await;
        send.finish().unwrap();
        assert!(callbacks.try_recv().is_err());
        assert_ne!(
            node.links().snapshot_or_never_seen(&peer.id).state,
            LinkState::Ready
        );
        gate.release.notify_one();
        let session = next_session(&mut callbacks).await;
        let mut incoming = session.accept_bi().await.unwrap();
        assert_eq!(
            incoming.read_opening().await.unwrap(),
            StreamOpening::Snapshot(b"req".to_vec())
        );
        incoming.expect_fin().await.unwrap();
        node.shutdown();
    }
    struct AdmissionGate {
        authority: Arc<TestAuthorizer>,
        calls: AtomicUsize,
        entered: tokio::sync::Notify,
        release: tokio::sync::Notify,
    }

    impl LinkAuthorizer for AdmissionGate {
        fn authorize<'a>(
            &'a self,
            local: &'a LinkIdentity,
            remote: &'a LinkIdentity,
        ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>>
        {
            self.authority.authorize(local, remote)
        }

        fn authorize_until<'a>(
            &'a self,
            local: &'a LinkIdentity,
            remote: &'a LinkIdentity,
            _deadline: tokio::time::Instant,
        ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>>
        {
            Box::pin(async move {
                if self.calls.fetch_add(1, Ordering::Relaxed) == 1 {
                    self.entered.notify_one();
                    self.release.notified().await;
                }
                self.authority.authorize(local, remote).await
            })
        }

        fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
            self.authority.subscribe()
        }
    }

    #[tokio::test]
    async fn fin_de_uma_stream_nao_interrompe_outra_em_admissao() {
        let peer = RawPeer::new(config());
        let gate = Arc::new(AdmissionGate {
            authority: TestAuthorizer::new(Ok(None)),
            calls: AtomicUsize::new(0),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        let (node, mut callbacks) =
            fixture_with_streams(&peer, gate.clone(), true, Duration::from_secs(3), 2);
        let connection = session_handshake(&node, &peer).await;
        let session = next_session(&mut callbacks).await;
        let (mut a, mut a_reply) = send_raw(&connection, b"\0\0\0\x04pull").await;
        a.finish().unwrap();
        let mut first = session.accept_bi().await.unwrap();
        assert_eq!(
            first.read_opening().await.unwrap(),
            StreamOpening::A2(b"pull".to_vec())
        );
        let (mut b, mut b_reply) = send_raw(&connection, b"\0\0\0\0\0\0\0\x03req").await;
        b.finish().unwrap();

        let (second, ()) = tokio::join!(session.accept_bi(), async {
            tokio::time::timeout(Duration::from_secs(2), gate.entered.notified())
                .await
                .unwrap();
            first.expect_fin().await.unwrap();
            first.write_frame(b"done").await.unwrap();
            first.finish_send().unwrap();
            drop(first);
            assert_eq!(a_reply.read_to_end(8).await.unwrap(), b"\0\0\0\x04done");
            gate.release.notify_one();
        });
        let mut second = second.unwrap();
        assert_eq!(
            second.read_opening().await.unwrap(),
            StreamOpening::Snapshot(b"req".to_vec())
        );
        second.expect_fin().await.unwrap();
        second.write_snapshot_control(b"result").await.unwrap();
        second.finish_send().unwrap();
        assert_eq!(b_reply.read_to_end(10).await.unwrap(), b"\0\0\0\x06result");
        node.shutdown();
    }
}

mod pinned_dial {
    use super::*;
    use godmesh_federation::node::{DialCancellation, DialError, DialTarget};
    use tokio::net::UdpSocket;

    fn target(peer: &Node, addr: SocketAddr) -> DialTarget {
        DialTarget {
            node_id: peer.id.clone(),
            public_key: peer.key,
            addr,
        }
    }

    struct DeadlineObserver {
        authority: Arc<PromotionGate>,
        calls: AtomicUsize,
    }

    impl LinkAuthorizer for DeadlineObserver {
        fn authorize<'a>(
            &'a self,
            local: &'a LinkIdentity,
            remote: &'a LinkIdentity,
        ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>>
        {
            self.authority.authorize(local, remote)
        }

        fn authorize_until<'a>(
            &'a self,
            local: &'a LinkIdentity,
            remote: &'a LinkIdentity,
            deadline: tokio::time::Instant,
        ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>>
        {
            let remaining = deadline.duration_since(tokio::time::Instant::now());
            assert!(remaining > Duration::from_secs(4) && remaining <= Duration::from_secs(5));
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.authority.authorize(local, remote)
        }

        fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
            self.authority.subscribe()
        }
    }

    #[tokio::test]
    async fn pin_errado_fecha_antes_do_manifesto() {
        let caller = node(config());
        let expected = node(config());
        let other = node(config());
        for peer in [&expected, &other] {
            caller
                .registry
                .insert(peer.record_for(TrustState::Authorized));
            peer.registry
                .insert(caller.record_for(TrustState::Authorized));
        }
        let mut attempt = caller
            .node
            .start_dial(target(&expected, other.addr()))
            .unwrap();
        assert!(matches!(attempt.wait().await, Err(DialError::PinMismatch)));
        assert_ne!(
            caller.links().snapshot_or_never_seen(&other.id).state,
            LinkState::Ready
        );
        assert_ne!(
            other.links().snapshot_or_never_seen(&caller.id).state,
            LinkState::Ready
        );
        assert_eq!(
            other
                .links()
                .snapshot_or_never_seen(&caller.id)
                .last_seen_at,
            None
        );
        for peer in [&caller, &expected, &other] {
            peer.node.shutdown();
        }
    }

    #[tokio::test]
    async fn recusa_explicita_preserva_evidencia_do_par_pinado() {
        let caller = node(config());
        let other = node(config());
        caller
            .registry
            .insert(other.record_for(TrustState::Authorized));
        other
            .registry
            .insert(caller.record_for(TrustState::Pending));
        let mut attempt = caller
            .node
            .start_dial(target(&other, other.addr()))
            .unwrap();
        assert!(matches!(
            attempt.wait().await,
            Err(DialError::Link(godmesh_federation::node::LinkError::Rejected { code, .. }))
                if code == Rejection::NotAuthorized.code()
        ));
        assert_eq!(
            caller.links().snapshot_or_never_seen(&other.id).state,
            LinkState::Rejected
        );
        caller.node.shutdown();
        other.node.shutdown();
    }

    #[tokio::test]
    async fn discador_recusa_manifesto_do_par_quando_diverge_do_tls() {
        let caller = node(config());
        let pinned = node(config());
        let another = node(config());
        caller
            .registry
            .insert(pinned.record_for(TrustState::Authorized));
        caller
            .registry
            .insert(another.record_for(TrustState::Authorized));
        let registry = Arc::new(PeerRegistry::new());
        registry.insert(caller.record_for(TrustState::Authorized));
        let tls = transport::NodeTls::self_issued(pinned.node.identity()).unwrap();
        let endpoint = transport::bind_server(
            ([127, 0, 0, 1], 0).into(),
            transport::server_config(registry, &tls).unwrap(),
        )
        .unwrap();
        let addr = endpoint.local_addr().unwrap();
        let reply = config()
            .signed_manifest(another.node.identity())
            .to_frame()
            .unwrap();
        let peer = tokio::spawn(async move {
            let connection = endpoint.accept().await.unwrap().await.unwrap();
            let (mut send, mut recv) = connection.accept_bi().await.unwrap();
            assert!(!recv.read_to_end(64 * 1024).await.unwrap().is_empty());
            send.write_all(&reply).await.unwrap();
            send.finish().unwrap();
            assert_eq!(close_code(&connection).await, Rejection::KeyMismatch.code());
        });
        let mut attempt = caller.node.start_dial(target(&pinned, addr)).unwrap();
        assert!(matches!(
            attempt.wait().await,
            Err(DialError::Link(godmesh_federation::node::LinkError::Rejected { code, .. }))
                if code == Rejection::KeyMismatch.code()
        ));
        assert_eq!(
            caller
                .links()
                .snapshot_or_never_seen(&pinned.id)
                .last_seen_at,
            None
        );
        peer.await.unwrap();
        for node in [&caller, &pinned, &another] {
            node.node.shutdown();
        }
    }

    #[tokio::test]
    async fn cancela_e_recolhe_discagem_em_tls_pendente() {
        let caller = node(config());
        let other = node(config());
        caller
            .registry
            .insert(other.record_for(TrustState::Authorized));
        let silent = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut attempt = caller
            .node
            .start_dial(target(&other, silent.local_addr().unwrap()))
            .unwrap();
        let mut packet = [0; 1500];
        tokio::time::timeout(Duration::from_secs(2), silent.recv_from(&mut packet))
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(attempt.cancel(), DialCancellation::Cancelled));
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), attempt.wait()).await,
            Ok(Err(DialError::Cancelled)),
        ));
        caller.node.shutdown();
        other.node.shutdown();
    }

    #[tokio::test]
    async fn cancelamento_ou_drop_apos_tls_impede_promocao() {
        for by_drop in [false, true] {
            let remote = node(config());
            let raw = RawPeer::new(config());
            let gate = Arc::new(PromotionGate {
                calls: AtomicUsize::new(0),
                entered: tokio::sync::Notify::new(),
                release: tokio::sync::Notify::new(),
                reject: false,
                authority: TestAuthorizer::new(Ok(None)),
            });
            let observer = Arc::new(DeadlineObserver {
                authority: gate.clone(),
                calls: AtomicUsize::new(0),
            });
            let (tx, mut callbacks) = tokio::sync::mpsc::unbounded_channel();
            let caller =
                bind_session_fixture(&raw, observer.clone(), config(), Arc::new(SessionSink(tx)));
            caller
                .registry()
                .insert(remote.record_for(TrustState::Authorized));
            remote.registry.insert(PeerRecord::authorized_from_key(
                &caller.identity().public_key_bytes(),
                CLUSTER,
            ));
            let mut attempt = caller.start_dial(target(&remote, remote.addr())).unwrap();
            tokio::time::timeout(Duration::from_secs(2), gate.entered.notified())
                .await
                .unwrap();
            assert_eq!(observer.calls.load(Ordering::Relaxed), 2);
            if by_drop {
                drop(attempt);
            } else {
                assert!(matches!(attempt.cancel(), DialCancellation::Cancelled));
                assert!(matches!(attempt.wait().await, Err(DialError::Cancelled)));
            }
            gate.release.notify_one();
            wait_for_state(
                &remote.links(),
                caller.identity().node_id(),
                LinkState::Unknown,
            )
            .await;
            assert!(callbacks.try_recv().is_err());
            assert_ne!(
                caller.links().snapshot_or_never_seen(&remote.id).state,
                LinkState::Ready
            );
            caller.shutdown();
            remote.node.shutdown();
        }
    }

    #[tokio::test]
    async fn promocao_vence_e_invalida_somente_a_conexao_original() {
        let (caller, other) = pair().await;
        let mut first = caller
            .node
            .start_dial(target(&other, other.addr()))
            .unwrap();
        assert_eq!(
            first.wait().await.unwrap().snapshot().state,
            LinkState::Ready
        );
        let mut second = caller
            .node
            .start_dial(target(&other, other.addr()))
            .unwrap();
        let current = second.wait().await.unwrap();
        let DialCancellation::AlreadyPromoted(original) = first.cancel() else {
            panic!("promoção deveria vencer");
        };
        original.invalidate();
        drop(first);
        assert_eq!(
            caller.links().snapshot_or_never_seen(&other.id).state,
            LinkState::Ready
        );
        assert_eq!(
            other.links().snapshot_or_never_seen(&caller.id).state,
            LinkState::Ready
        );
        current.invalidate();
        wait_for_state(&caller.links(), &other.id, LinkState::Unknown).await;
        caller.node.shutdown();
        other.node.shutdown();
    }
    #[tokio::test]
    async fn permit_que_fica_pronto_apos_prazo_nao_promove() {
        struct LatePermit(tokio::sync::watch::Sender<u64>);
        impl LinkAuthorizer for LatePermit {
            fn authorize<'a>(
                &'a self,
                _: &'a LinkIdentity,
                _: &'a LinkIdentity,
            ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>>
            {
                Box::pin(async { Ok(LinkPermit::new(Box::new(()), None)) })
            }

            fn authorize_until<'a>(
                &'a self,
                local: &'a LinkIdentity,
                remote: &'a LinkIdentity,
                deadline: tokio::time::Instant,
            ) -> Pin<Box<dyn Future<Output = Result<LinkPermit, LinkAuthorizationError>> + Send + 'a>>
            {
                Box::pin(async move {
                    // Simula uma consulta que devolve o resultado no primeiro poll
                    // após a pausa do executor, antes de o timer poder ser observado.
                    std::thread::sleep(
                        deadline.saturating_duration_since(tokio::time::Instant::now())
                            + Duration::from_millis(10),
                    );
                    self.authorize(local, remote).await
                })
            }

            fn subscribe(&self) -> tokio::sync::watch::Receiver<u64> {
                self.0.subscribe()
            }
        }

        let remote = node(config());
        let raw = RawPeer::new(config());
        let (tx, mut callbacks) = tokio::sync::mpsc::unbounded_channel();
        let caller = bind_session_fixture(
            &raw,
            Arc::new(LatePermit(tokio::sync::watch::channel(0).0)),
            config(),
            Arc::new(SessionSink(tx)),
        );
        caller
            .registry()
            .insert(remote.record_for(TrustState::Authorized));
        remote.registry.insert(PeerRecord::authorized_from_key(
            &caller.identity().public_key_bytes(),
            CLUSTER,
        ));
        let mut attempt = caller.start_dial(target(&remote, remote.addr())).unwrap();
        assert!(matches!(
            attempt.wait().await,
            Err(DialError::Link(godmesh_federation::node::LinkError::Rejected { code, .. }))
                if code == Rejection::NotAuthorized.code()
        ));
        assert!(callbacks.try_recv().is_err());
        assert_ne!(
            caller.links().snapshot_or_never_seen(&remote.id).state,
            LinkState::Ready
        );
        caller.shutdown();
        remote.node.shutdown();
    }

    #[tokio::test]
    async fn promocao_operacional_e_recusa_preservam_sessao_substituta() {
        let other = node(config());
        let raw = RawPeer::new(config());
        let (tx, mut callbacks) = tokio::sync::mpsc::unbounded_channel();
        let caller = bind_session_fixture(
            &raw,
            TestAuthorizer::new(Ok(None)),
            config(),
            Arc::new(SessionSink(tx)),
        );
        caller
            .registry()
            .insert(other.record_for(TrustState::Authorized));
        let caller_key = caller.identity().public_key_bytes();
        other
            .registry
            .insert(PeerRecord::authorized_from_key(&caller_key, CLUSTER));
        let mut first = caller.start_dial(target(&other, other.addr())).unwrap();
        first.wait().await.unwrap();
        let old_session = next_session(&mut callbacks).await;
        let mut second = caller.start_dial(target(&other, other.addr())).unwrap();
        let current = second.wait().await.unwrap();
        let new_session = next_session(&mut callbacks).await;
        assert_ne!(old_session.id(), new_session.id());

        let refusing_registry = Arc::new(PeerRegistry::new());
        refusing_registry.insert(
            PeerRecord::authorized_from_key(&caller_key, CLUSTER)
                .with_trust_state(TrustState::Pending),
        );
        let tls = transport::NodeTls::self_issued(other.node.identity()).unwrap();
        let endpoint = transport::bind_server(
            ([127, 0, 0, 1], 0).into(),
            transport::server_config(Arc::clone(&refusing_registry), &tls).unwrap(),
        )
        .unwrap();
        let refusing_addr = endpoint.local_addr().unwrap();
        let peer_identity = NodeIdentity::generate_in_memory().unwrap();
        let rejecting_peer = tokio::spawn(async move {
            let connection = endpoint.accept().await.unwrap().await.unwrap();
            let rejected =
                handshake::accept(&connection, &peer_identity, &config(), &refusing_registry)
                    .await
                    .unwrap_err();
            assert_eq!(rejected.rejection, Rejection::NotAuthorized);
        });
        let mut refused = caller.start_dial(target(&other, refusing_addr)).unwrap();
        assert!(matches!(
            refused.wait().await,
            Err(DialError::Link(
                godmesh_federation::node::LinkError::Rejected { .. }
            ))
        ));
        assert_eq!(
            caller.links().snapshot_or_never_seen(&other.id).state,
            LinkState::Ready
        );

        rejecting_peer.await.unwrap();
        let DialCancellation::AlreadyPromoted(old) = first.cancel() else {
            panic!("promoção venceu")
        };
        old.invalidate();
        tokio::time::timeout(Duration::from_secs(2), old_session.cancelled())
            .await
            .unwrap();
        assert!(new_session.open_bi(attempt_deadline()).await.is_ok());
        assert_eq!(
            caller.links().snapshot_or_never_seen(&other.id).state,
            LinkState::Ready
        );
        current.invalidate();
        tokio::time::timeout(Duration::from_secs(2), new_session.cancelled())
            .await
            .unwrap();
        caller.shutdown();
        other.node.shutdown();
    }
}

#[cfg(feature = "relay-harness")]
fn relay_control(kind: &str, route: &str, token: [u8; 16]) -> Vec<u8> {
    let mut json = serde_json::json!({
        "type": format!("m5.relay.{kind}.v1"),
        "request_id": "0123456789abcdef0123456789abcdef",
        "route_id": route,
    });
    if kind == "accept" {
        json["ready_token"] = hex::encode(token).into();
        json["max_datagram_bytes"] = 1400.into();
    }
    if kind == "refuse" {
        json["error"] = "route_denied".into();
    }
    serde_json::to_vec(&json).unwrap()
}

#[cfg(feature = "relay-harness")]
async fn relay_raw_write(send: &mut quinn::SendStream, body: &[u8]) {
    send.write_all(&u32::try_from(body.len()).unwrap().to_be_bytes())
        .await
        .unwrap();
    send.write_all(body).await.unwrap();
}

#[cfg(feature = "relay-harness")]
async fn relay_raw_read(recv: &mut quinn::RecvStream, expected: &[u8]) {
    let mut bytes = vec![0; expected.len() + 4];
    recv.read_exact(&mut bytes).await.unwrap();
    assert_eq!(
        &bytes[..4],
        &u32::try_from(expected.len()).unwrap().to_be_bytes()
    );
    assert_eq!(&bytes[4..], expected);
}

#[cfg(feature = "relay-harness")]
#[tokio::test]
async fn relay_rejeita_prefixo_sem_corpo_sem_fechar_sessao() {
    let peer = RawPeer::new(config());
    let (node, mut sessions) =
        session_node_with_config(&peer, TestAuthorizer::new(Ok(None)), config());
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut sessions).await;
    let (mut send, _recv) = connection.open_bi().await.unwrap();
    send.write_all(&513_u32.to_be_bytes()).await.unwrap();
    let mut opening = session
        .accept_relay(Duration::from_secs(2), NonZeroU32::new(512).unwrap())
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_millis(500), opening.read_control())
            .await
            .unwrap(),
        Err(SessionError::InvalidFrame)
    );

    drop(opening);

    let (mut send, _recv) = connection.open_bi().await.unwrap();
    let request = relay_control("open", "d1", [0; 16]);
    relay_raw_write(&mut send, &request).await;
    let mut opening = session
        .accept_relay(Duration::from_secs(2), NonZeroU32::new(512).unwrap())
        .await
        .unwrap();
    assert_eq!(opening.read_control().await.unwrap(), request);
}

#[cfg(feature = "relay-harness")]
#[tokio::test]
async fn relay_token_preserva_frame_e_prazo_de_abertura_nao_expira_encaminhamento() {
    let peer = RawPeer::new(config());
    let (node, mut sessions) =
        session_node_with_config(&peer, TestAuthorizer::new(Ok(None)), config());
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut sessions).await;
    let cap = NonZeroU32::new(512).unwrap();
    let (mut send, mut recv) = connection.open_bi().await.unwrap();
    let request = relay_control("open", "d1", [0; 16]);
    relay_raw_write(&mut send, &request).await;
    let mut opening = session
        .accept_relay(Duration::from_secs(1), cap)
        .await
        .unwrap();
    assert_eq!(opening.read_control().await.unwrap(), request);
    let token = [42; 16];
    let reply = relay_control("accept", "d1", token);
    opening.write_control(&reply).await.unwrap();
    relay_raw_read(&mut recv, &reply).await;
    send.write_all(&token[..5]).await.unwrap();
    let body = vec![27; 1400];
    let mut suffix = token[5..].to_vec();
    suffix.extend_from_slice(&1400_u32.to_be_bytes());
    suffix.extend_from_slice(&body);
    send.write_all(&suffix).await.unwrap();
    opening.expect_token(token).await.unwrap();
    let opening_deadline = opening.deadline();
    let (mut outbound, mut inbound, cancel) = opening
        .into_forwarding(NonZeroU32::new(1400).unwrap())
        .unwrap();
    tokio::time::sleep_until(opening_deadline).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
    assert_eq!(inbound.read_datagram(deadline).await.unwrap(), body);
    assert_eq!(
        outbound.write_datagram(&[0; 1401], deadline).await,
        Err(SessionError::InvalidFrame)
    );
    outbound.write_datagram(&body, deadline).await.unwrap();
    let mut framed = vec![0; 1404];
    recv.read_exact(&mut framed).await.unwrap();
    assert_eq!(&framed[..4], &1400_u32.to_be_bytes());
    assert_eq!(&framed[4..], body);

    let read = inbound.read_datagram(deadline);
    let cancel_read = async {
        tokio::task::yield_now().await;
        cancel.cancel();
    };
    assert_eq!(tokio::join!(read, cancel_read).0, Err(SessionError::Closed));
    assert!(matches!(
        tokio::time::timeout(Duration::from_millis(500), recv.read(&mut [0])).await,
        Ok(Err(quinn::ReadError::Reset(_)))
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(500), send.stopped())
            .await
            .unwrap()
            .unwrap()
            .is_some()
    );
    assert!(matches!(
        session.open_relay(deadline, cap).await,
        Err(SessionError::Busy)
    ));
    drop(inbound);
    drop(outbound);

    let mut next = session.open_relay(deadline, cap).await.unwrap();
    next.write_control(&request).await.unwrap();
    let (mut peer_send, mut peer_recv) = connection.accept_bi().await.unwrap();
    relay_raw_read(&mut peer_recv, &request).await;
    relay_raw_write(&mut peer_send, &reply).await;
    assert_eq!(next.read_control().await.unwrap(), reply);
    next.send_token(token).await.unwrap();
    let mut confirmation = [0; 16];
    peer_recv.read_exact(&mut confirmation).await.unwrap();
    assert_eq!(confirmation, token);
}

#[cfg(feature = "relay-harness")]
#[tokio::test]
async fn relay_token_divergente_ou_incompleto_encerra_abertura() {
    let peer = RawPeer::new(config());
    let (node, mut sessions) =
        session_node_with_config(&peer, TestAuthorizer::new(Ok(None)), config());
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut sessions).await;
    let cap = NonZeroU32::new(512).unwrap();

    for (token, error) in [
        (&[1; 16][..], SessionError::InvalidFrame),
        (&[0; 15][..], SessionError::Timeout),
    ] {
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        let request = relay_control("open", "d1", [0; 16]);
        relay_raw_write(&mut send, &request).await;
        let mut opening = session
            .accept_relay(Duration::from_millis(150), cap)
            .await
            .unwrap();
        assert_eq!(opening.read_control().await.unwrap(), request);
        let reply = relay_control("accept", "d1", [0; 16]);
        opening.write_control(&reply).await.unwrap();
        relay_raw_read(&mut recv, &reply).await;
        send.write_all(token).await.unwrap();
        assert_eq!(opening.expect_token([0; 16]).await, Err(error));
        assert!(opening.into_forwarding(cap).is_err());
        assert!(matches!(
            recv.read(&mut [0]).await,
            Err(quinn::ReadError::Reset(_))
        ));
    }
}

#[cfg(feature = "relay-harness")]
#[tokio::test]
async fn relay_recusa_exige_fin_sem_bytes_adicionais() {
    let peer = RawPeer::new(config());
    let (node, mut sessions) =
        session_node_with_config(&peer, TestAuthorizer::new(Ok(None)), config());
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut sessions).await;
    let cap = NonZeroU32::new(512).unwrap();

    for extra in [false, true] {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        let mut opening = session.open_relay(deadline, cap).await.unwrap();
        opening
            .write_control(&relay_control("open", "d1", [0; 16]))
            .await
            .unwrap();
        let (mut send, _recv) = connection.accept_bi().await.unwrap();
        let refusal = relay_control("refuse", "d1", [0; 16]);
        relay_raw_write(&mut send, &refusal).await;
        if extra {
            send.write_all(b"x").await.unwrap();
        }
        send.finish().unwrap();
        assert_eq!(opening.read_control().await.unwrap(), refusal);
        assert_eq!(
            opening.expect_fin().await,
            if extra {
                Err(SessionError::InvalidFrame)
            } else {
                Ok(())
            }
        );
    }
}

#[cfg(feature = "relay-harness")]
#[tokio::test]
async fn relay_dados_rejeitam_prefixo_excedente_e_respeitam_prazo_proprio() {
    let peer = RawPeer::new(config());
    let (node, mut sessions) =
        session_node_with_config(&peer, TestAuthorizer::new(Ok(None)), config());
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut sessions).await;
    let control_cap = NonZeroU32::new(512).unwrap();
    let datagram_cap = NonZeroU32::new(1400).unwrap();

    for (length, error) in [
        (1401_u32, SessionError::InvalidFrame),
        (1400_u32, SessionError::Timeout),
    ] {
        let (mut send, mut recv) = connection.open_bi().await.unwrap();
        let request = relay_control("open", "d1", [0; 16]);
        relay_raw_write(&mut send, &request).await;
        let mut opening = session
            .accept_relay(Duration::from_secs(2), control_cap)
            .await
            .unwrap();
        assert_eq!(opening.read_control().await.unwrap(), request);
        let reply = relay_control("accept", "d1", [0; 16]);
        opening.write_control(&reply).await.unwrap();
        relay_raw_read(&mut recv, &reply).await;
        send.write_all(&[0; 16]).await.unwrap();
        send.write_all(&length.to_be_bytes()).await.unwrap();
        opening.expect_token([0; 16]).await.unwrap();
        let (mut outbound, mut inbound, cancel) = opening.into_forwarding(datagram_cap).unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(150);
        assert_eq!(inbound.read_datagram(deadline).await, Err(error));
        cancel.cancelled().await;
        assert_eq!(
            outbound
                .write_datagram(b"x", tokio::time::Instant::now() + Duration::from_secs(1))
                .await,
            Err(SessionError::Closed)
        );
        assert!(matches!(
            recv.read(&mut [0]).await,
            Err(quinn::ReadError::Reset(_))
        ));
    }
}

#[cfg(feature = "relay-harness")]
#[tokio::test]
async fn relay_barreira_exige_aceite_correlacionado_nas_duas_pontas() {
    let peer = RawPeer::new(config());
    let (node, mut sessions) =
        session_node_with_config(&peer, TestAuthorizer::new(Ok(None)), config());
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut sessions).await;
    let cap = NonZeroU32::new(512).unwrap();
    let token = [42; 16];
    let request = relay_control("open", "d1", token);

    for send_open in [false, true] {
        let (mut send, _recv) = connection.open_bi().await.unwrap();
        if send_open {
            relay_raw_write(&mut send, &request).await;
        }
        send.write_all(&token).await.unwrap();
        let mut opening = session
            .accept_relay(Duration::from_secs(2), cap)
            .await
            .unwrap();
        if send_open {
            assert_eq!(opening.read_control().await.unwrap(), request);
        }
        assert_eq!(
            opening.expect_token(token).await,
            Err(SessionError::InvalidFrame)
        );
    }

    for unsolicited in [false, true] {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(1);
        let mut opening = session.open_relay(deadline, cap).await.unwrap();
        opening.write_control(&request).await.unwrap();
        let (mut send, mut recv) = connection.accept_bi().await.unwrap();
        relay_raw_read(&mut recv, &request).await;
        if unsolicited {
            relay_raw_write(&mut send, &relay_control("accept", "d2", token)).await;
            assert_eq!(
                opening.read_control().await,
                Err(SessionError::InvalidFrame)
            );
        } else {
            assert_eq!(
                opening.send_token(token).await,
                Err(SessionError::InvalidFrame)
            );
        }
    }
}

#[cfg(feature = "relay-harness")]
#[tokio::test]
async fn relay_cancelamento_fecha_as_duas_metades_ociosas_antes_do_recolhimento() {
    let peer = RawPeer::new(config());
    let (node, mut sessions) =
        session_node_with_config(&peer, TestAuthorizer::new(Ok(None)), config());
    let connection = session_handshake(&node, &peer).await;
    let session = next_session(&mut sessions).await;
    let cap = NonZeroU32::new(512).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let token = [42; 16];
    let request = relay_control("open", "d1", token);
    let reply = relay_control("accept", "d1", token);
    let mut opening = session.open_relay(deadline, cap).await.unwrap();
    opening.write_control(&request).await.unwrap();
    let (mut send, mut recv) = connection.accept_bi().await.unwrap();
    relay_raw_read(&mut recv, &request).await;
    relay_raw_write(&mut send, &reply).await;
    assert_eq!(opening.read_control().await.unwrap(), reply);
    opening.send_token(token).await.unwrap();
    let mut confirmation = [0; 16];
    recv.read_exact(&mut confirmation).await.unwrap();
    assert_eq!(confirmation, token);
    let (outbound, inbound, cancel) = opening
        .into_forwarding(NonZeroU32::new(1400).unwrap())
        .unwrap();

    cancel.cancel();
    tokio::time::timeout(Duration::from_millis(500), async {
        assert!(matches!(
            recv.read(&mut [0]).await,
            Err(quinn::ReadError::Reset(_))
        ));
        assert!(send.stopped().await.unwrap().is_some());
    })
    .await
    .unwrap();
    drop(outbound);
    assert!(matches!(
        session.open_relay(deadline, cap).await,
        Err(SessionError::Busy)
    ));
    drop(inbound);
    assert!(session.open_relay(deadline, cap).await.is_ok());
}
