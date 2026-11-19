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
    let mut outgoing = session.open_bi().await.unwrap();
    assert!(matches!(session.open_bi().await, Err(SessionError::Busy)));
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
    assert!(session.open_bi().await.is_ok());
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
    assert!(matches!(old.open_bi().await, Err(SessionError::Closed)));
    let mut active = current.open_bi().await.unwrap();
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
        let mut stream = session.open_bi().await.unwrap();
        stream.write_frame(b"pull").await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), stream.read_frame())
                .await
                .unwrap(),
            Err(SessionError::Closed),
        );
        session.cancelled().await;
        assert!(matches!(session.open_bi().await, Err(SessionError::Closed)));
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
