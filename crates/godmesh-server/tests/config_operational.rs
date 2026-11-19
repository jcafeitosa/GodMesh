use godmesh_server::config_operational::ConfigPlaneConfig;

#[test]
fn ambiente_incompleto_nao_desabilita_plano_por_omissao() {
    assert!(
        ConfigPlaneConfig::from_env_with(|_| None)
            .unwrap()
            .is_none()
    );
    let partial = |key: &str| (key == "GODMESH_CONFIG_MAX_ATTEMPTS").then(|| "3".into());
    assert!(ConfigPlaneConfig::from_env_with(partial).is_err());
}

mod common;

use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Extension,
    http::{HeaderMap, HeaderValue, StatusCode, header, uri::PathAndQuery},
};
use godmesh_federation::{
    NodeIdentity,
    config_bundle::{BundleBody, BundleKey, SoftwareVersion, vote_bytes},
};
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::{
    admission::AdmissionConfig,
    auth::{Authorization, FixedClock, GrantRecord, GrantRegistry, Scope, Subject},
    config_bundle::{BundleService, Preflight, Role, WriteOutcome},
    config_operational::{
        ConfigPlane, Peer, Publisher, PullClient, PullDenial, PullError, PullFuture, PullResponse,
        PullResult, TransportError,
    },
    portable_join::{PortableJoin, PortableJoinConfig},
};
use serde_json::json;
use sqlx::PgPool;

async fn database(suffix: &str) -> Option<PgPool> {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    let base = common::database_url()?.parse::<PgConnectOptions>().unwrap();
    let admin = PgPoolOptions::new()
        .connect_with(base.clone().database("postgres"))
        .await
        .unwrap();
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(version.parse::<u32>().unwrap() / 10_000, 16);
    let name = format!("b2_{}_{suffix}", std::process::id());
    assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .connect_with(base.database(&name))
        .await
        .unwrap();
    godmesh_server::migrate::migrate(&pool, &common::migrations_dir())
        .await
        .unwrap();
    Some(pool)
}

fn pin(id: &NodeIdentity) -> String {
    format!("{}:{}", id.node_id(), id.public_key_hex())
}

fn env(
    role: Role,
    voters: &[NodeIdentity],
    ids: &[&NodeIdentity],
    url: &str,
) -> BTreeMap<String, String> {
    let mut cfg: BTreeMap<String, String> = [
        (
            "ROLE",
            if role == Role::Publisher {
                "publisher"
            } else {
                "receiver"
            },
        ),
        ("CLUSTER", "lab"),
        ("EPOCH", "7"),
        ("SOFTWARE_VERSION", "1.0.0"),
        ("MAX_FRAME_BYTES", "8192"),
        ("BIND", "127.0.0.1:0"),
        ("HANDSHAKE_LIMIT", "4"),
        ("CONNECTION_LIMIT", "8"),
        ("HANDSHAKE_TIMEOUT_MS", "2000"),
        ("HEADER_TIMEOUT_MS", "2000"),
        ("MAX_BODY_BYTES", "8192"),
        ("BODY_TIMEOUT_MS", "2000"),
        ("PULL_TIMEOUT_MS", "2000"),
        ("POLL_INTERVAL_MS", "10000"),
        ("MAX_ATTEMPTS", "3"),
        ("RETRY_INTERVAL_MS", "1"),
        ("LINK_RECHECK_INTERVAL_MS", "1000"),
    ]
    .into_iter()
    .map(|(key, value)| (format!("GODMESH_CONFIG_{key}"), value.into()))
    .collect();
    let mut put = |key, value| {
        cfg.insert(format!("GODMESH_CONFIG_{key}"), value);
    };
    let grants = ids.iter().map(|id| pin(id)).collect::<Vec<_>>().join(",");
    for key in ["READERS", "SUBMITTERS", "RESOLVERS"] {
        put(key, grants.clone());
    }
    put(
        "VOTERS",
        voters.iter().map(pin).collect::<Vec<_>>().join(","),
    );
    if role == Role::Receiver {
        put("PUBLISHER_URL", url.into());
        put("PUBLISHER_PIN", pin(ids[0]));
    }
    cfg
}

fn portable(
    pool: PgPool,
    identity: Arc<NodeIdentity>,
    genesis: &[&NodeIdentity],
) -> Arc<PortableJoin> {
    let genesis = genesis
        .iter()
        .map(|id| pin(id))
        .collect::<Vec<_>>()
        .join(",");
    let config = PortableJoinConfig::from_env_with(|key| {
        Some(
            match key {
                "GODMESH_FEDERATION_GOVERNANCE" => "single_approver:lab",
                "GODMESH_FEDERATION_GENESIS" => &genesis,
                "GODMESH_FEDERATION_REGION" => "test",
                "GODMESH_FEDERATION_ADVERTISE_QUIC" => "quic://127.0.0.1:4433",
                "GODMESH_FEDERATION_PORTABLE_MAX_DEPTH" => "4",
                "GODMESH_FEDERATION_PORTABLE_MAX_FRAME_BYTES" => "8192",
                "GODMESH_FEDERATION_PORTABLE_MAX_VALIDITY_SECONDS" => "60",
                _ => return None,
            }
            .into(),
        )
    })
    .unwrap()
    .unwrap();
    Arc::new(PortableJoin::new(config, pool, identity))
}

fn make_grant(scope: Scope, token_id: &str) -> GrantRecord {
    GrantRecord {
        token_id: token_id.into(),
        subject: Subject {
            tenant_id: "lab".into(),
            principal: "operador".into(),
        },
        scopes: vec![scope],
        not_before: 0,
        expires_at: 150,
        revoked_at: None,
        authority_epoch: 1,
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

async fn plane(
    portal: Arc<PortableJoin>,
    identity: Arc<NodeIdentity>,
    settings: ConfigPlaneConfig,
) -> Arc<ConfigPlane> {
    let pool = portal.pool();
    let Preflight::Enabled(service) =
        BundleService::preflight(pool, Some(&portal), Some(settings.policy.clone()))
            .await
            .unwrap()
    else {
        panic!("B1 desabilitada")
    };
    let auth = Authorization::new(
        GrantRegistry::from_records([
            (
                "config-admin-test".into(),
                make_grant(Scope::FederationAdmin, "cfg-test-admin"),
            ),
            (
                "config-read-test".into(),
                make_grant(Scope::FederationRead, "cfg-test-read"),
            ),
        ]),
        pool.clone(),
    );
    Arc::new(
        ConfigPlane::new(
            settings,
            Arc::new(service),
            portal,
            identity,
            Arc::new(auth),
            AdmissionConfig::default(),
        )
        .unwrap()
        .with_clock(Arc::new(FixedClock(100))),
    )
}

fn signed(voters: &[NodeIdentity], version: u64, floor: u64, requires: u64) -> (Vec<u8>, String) {
    let body = BundleBody {
        key: BundleKey {
            cluster: "lab".into(),
            namespace: "federation".into(),
            scope_id: String::new(),
            epoch: 7,
        },
        version,
        applies_at: 100,
        expires_at: 200,
        required_signatures: 3,
        value: floor,
        effective_after: 100,
        min_version: SoftwareVersion::new(requires, 0, 0).unwrap(),
    };
    let digest = body.digest();
    let hash = hex::encode(digest);
    let signatures: Vec<_> = voters
        .iter()
        .map(|id| {
            json!({
                "signer_node_id":id.node_id(), "signature":id.sign_hex(&vote_bytes(&digest))
            })
        })
        .collect();
    (serde_json::to_vec(&json!({
        "profile":1,"cluster":"lab","namespace":"federation","scope_id":"","epoch":7,
        "version":version,"applies_at":100,"expires_at":200,"required_signatures":3,
        "entries":[{"key":"federation.minimum_protocol_version","value":floor,"effective_after":100,"min_version":format!("{requires}.0.0")}],
        "hash":hash,"signatures":signatures
    })).unwrap(),hash)
}

fn peer(id: &NodeIdentity) -> Peer {
    Peer {
        node_id: id.node_id().into(),
        public_key: id.public_key_bytes(),
        remote_addr: "127.0.0.1:1234".parse().unwrap(),
    }
}

async fn server(
    plane: &Arc<ConfigPlane>,
    who: Option<Peer>,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let mut router = plane.router();
    if let Some(who) = who {
        router = router.layer(Extension(who));
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (addr, task)
}

#[derive(Clone)]
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn error(&self) -> String {
        serde_json::from_slice::<serde_json::Value>(&self.body).unwrap()["error"]
            .as_str()
            .unwrap()
            .into()
    }
}

const ROUTE: &str = "/v1/federation/config";
const GET: &str = "/v1/federation/config?cluster=lab&namespace=federation&scope_id=";

async fn call(
    addr: SocketAddr,
    method: &'static str,
    path: &str,
    headers: &[(&str, &str)],
    payload: &[u8],
) -> Reply {
    let mut raw = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    for (key, value) in headers {
        raw.push_str(&format!("{key}: {value}\r\n"));
    }
    if method == "PUT" {
        raw.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            payload.len()
        ));
    }
    raw.push_str("\r\n");
    let mut request = raw.into_bytes();
    request.extend_from_slice(payload);
    raw_call(addr, request).await
}

async fn raw_call(addr: SocketAddr, request: Vec<u8>) -> Reply {
    tokio::task::spawn_blocking(move || {
        let mut socket = std::net::TcpStream::connect(addr).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        socket.write_all(&request).unwrap();
        let mut response = Vec::new();
        socket.read_to_end(&mut response).unwrap();
        let split = response.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
        let mut lines = std::str::from_utf8(&response[..split])
            .unwrap()
            .split("\r\n");
        let status = StatusCode::from_u16(
            lines
                .next()
                .unwrap()
                .split(' ')
                .nth(1)
                .unwrap()
                .parse()
                .unwrap(),
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        for line in lines {
            let (key, value) = line.split_once(':').unwrap();
            headers.append(
                axum::http::HeaderName::from_bytes(key.as_bytes()).unwrap(),
                HeaderValue::from_str(value.trim()).unwrap(),
            );
        }
        Reply {
            status,
            headers,
            body: response[split + 4..].to_vec(),
        }
    })
    .await
    .unwrap()
}

async fn snapshot(pool: &PgPool) -> (i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM federation_config_frame), (SELECT count(*) FROM federation_config_audit)")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn api_confirma_antes_de_responder_e_preserva_etag_no_replay() {
    let Some(pool) = database("api").await else {
        return;
    };
    let who = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let voters: Vec<_> = (0..3)
        .map(|_| NodeIdentity::generate_in_memory().unwrap())
        .collect();
    let cfg = env(Role::Publisher, &voters, &[&who], "");
    let cfg = ConfigPlaneConfig::from_env_with(|key| cfg.get(key).cloned())
        .unwrap()
        .unwrap();
    let portal = portable(pool.clone(), who.clone(), &[&who]);
    let p = plane(portal, who.clone(), cfg).await;
    let (addr, task) = server(&p, Some(peer(&who))).await;
    let (frame, hash) = signed(&voters, 1, 1, 1);
    let (different, _) = signed(&voters, 1, 2, 1);
    let (future, _) = signed(&voters, 2, 2, 2);
    let (correct, correct_hash) = signed(&voters, 3, 1, 1);
    let admin = [("Authorization", "Bearer config-admin-test")];
    assert_eq!(
        call(addr, "GET", GET, &[], &[]).await.error(),
        "config_unavailable"
    );
    assert_eq!(call(addr, "PUT", ROUTE, &[], &frame).await.status, 401);
    assert_eq!(
        call(
            addr,
            "PUT",
            ROUTE,
            &[("Authorization", "Bearer config-read-test")],
            &frame
        )
        .await
        .status,
        403
    );
    assert_eq!(
        call(addr, "GET", &format!("{GET}&scope_id="), &[], &[])
            .await
            .status,
        400
    );
    let mut changed = p.changes();
    assert_eq!(call(addr, "PUT", ROUTE, &admin, &frame).await.status, 201);
    assert!(changed.has_changed().unwrap());
    changed.borrow_and_update();
    let stored = snapshot(&pool).await;
    assert_eq!(call(addr, "PUT", ROUTE, &admin, &frame).await.status, 200);
    assert_eq!(snapshot(&pool).await, stored);
    assert!(!changed.has_changed().unwrap());
    let returned = call(addr, "GET", GET, &[("X-Node-Id", "untrusted")], &[]).await;
    let tag = format!("\"{hash}\"");
    assert_eq!(returned.status, 200);
    assert_eq!(returned.headers[header::ETAG], tag);
    assert_eq!(
        returned.headers[header::CONTENT_LENGTH],
        frame.len().to_string()
    );
    assert_eq!(returned.body, frame);
    let cached = call(addr, "GET", GET, &[("If-None-Match", &tag)], &[]).await;
    assert_eq!(cached.status, 304);
    assert_eq!(cached.headers[header::ETAG], tag);
    assert!(cached.body.is_empty());
    assert_eq!(
        call(addr, "GET", GET, &[("If-None-Match", "*")], &[])
            .await
            .status,
        400
    );
    assert_eq!(
        call(
            addr,
            "GET",
            GET,
            &[("If-None-Match", &tag), ("If-None-Match", &tag)],
            &[]
        )
        .await
        .status,
        400
    );
    assert_eq!(
        call(addr, "PUT", ROUTE, &admin, &future).await.error(),
        "config_incompatible_software"
    );
    assert_eq!(snapshot(&pool).await, (2, 2));
    assert_eq!(
        call(addr, "PUT", ROUTE, &admin, &different).await.error(),
        "config_conflict"
    );
    assert_eq!(snapshot(&pool).await, (3, 3));
    assert_eq!(
        call(addr, "GET", GET, &[("If-None-Match", &tag)], &[])
            .await
            .status,
        503
    );
    let resolve = format!("{ROUTE}?resolve_conflict=true&expected_hash={correct_hash}");
    assert_eq!(
        call(addr, "PUT", &resolve, &admin, &correct).await.status,
        201
    );
    assert_eq!(
        call(addr, "PUT", &resolve, &admin, &correct).await.status,
        200
    );
    assert_eq!(call(addr, "GET", GET, &[], &[]).await.body, correct);
    let meta = call(addr, "GET", "/openapi.json", &[], &[]).await;
    let meta: serde_json::Value = serde_json::from_slice(&meta.body).unwrap();
    assert_eq!(
        meta["paths"][ROUTE]["get"]["security"][0]["peer"],
        json!([])
    );
    assert!(
        meta["paths"][ROUTE]["get"]["responses"]
            .get("304")
            .is_some()
    );
    let (untrusted, untrusted_task) = server(&p, None).await;
    assert_eq!(
        call(untrusted, "GET", GET, &[("X-Node-Id", who.node_id())], &[])
            .await
            .status,
        403
    );
    untrusted_task.abort();
    task.abort();
}

#[tokio::test]
async fn api_falha_no_commit_nao_confirma_evidencia() {
    let Some(pool) = database("commit").await else {
        return;
    };
    let who = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let voters: Vec<_> = (0..3)
        .map(|_| NodeIdentity::generate_in_memory().unwrap())
        .collect();
    let cfg = env(Role::Publisher, &voters, &[&who], "");
    let cfg = ConfigPlaneConfig::from_env_with(|key| cfg.get(key).cloned())
        .unwrap()
        .unwrap();
    let p = plane(
        portable(pool.clone(), who.clone(), &[&who]),
        who.clone(),
        cfg,
    )
    .await;
    let (addr, task) = server(&p, Some(peer(&who))).await;
    let admin = [("Authorization", "Bearer config-admin-test")];
    let (frame, _) = signed(&voters, 1, 1, 1);
    let (conflict, _) = signed(&voters, 1, 2, 1);
    assert_eq!(call(addr, "PUT", ROUTE, &admin, &frame).await.status, 201);
    let before = snapshot(&pool).await;
    sqlx::raw_sql(
        "CREATE FUNCTION reject_config_commit() RETURNS trigger LANGUAGE plpgsql AS
        'BEGIN RAISE EXCEPTION ''fixture de falha de commit''; END';
        CREATE CONSTRAINT TRIGGER reject_config_commit AFTER INSERT ON federation_config_audit
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_config_commit();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let result = call(addr, "PUT", ROUTE, &admin, &conflict).await;
    assert_eq!(result.status, 503);
    assert_eq!(result.error(), "storage_unavailable");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&result.body).unwrap()["effect"],
        "unknown"
    );
    assert_eq!(snapshot(&pool).await, before);
    assert_eq!(call(addr, "GET", GET, &[], &[]).await.body, frame);
    task.abort();
}

#[test]
fn origem_https_so_aceita_authority_e_porta_representavel() {
    let id = NodeIdentity::generate_in_memory().unwrap();
    let voters: Vec<_> = (0..3)
        .map(|_| NodeIdentity::generate_in_memory().unwrap())
        .collect();
    for url in [
        "http://127.0.0.1",
        "https://user@127.0.0.1",
        "https://127.0.0.1/",
        "https://127.0.0.1?x=1",
        "https://127.0.0.1#fragmento",
        "https://127.0.0.1:0",
        "https://127.0.0.1:65536",
        "https://127.0.0.1:",
    ] {
        let cfg = env(Role::Receiver, &voters, &[&id], url);
        assert!(
            ConfigPlaneConfig::from_env_with(|key| cfg.get(key).cloned()).is_err(),
            "{url}"
        );
    }
    let cfg = env(Role::Receiver, &voters, &[&id], "https://[::1]:4433");
    let cfg = ConfigPlaneConfig::from_env_with(|key| cfg.get(key).cloned())
        .unwrap()
        .unwrap();
    assert_eq!(cfg.publisher().unwrap().authority.port_u16(), Some(4433));
    assert_eq!(cfg.tls_pins(), [id.public_key_bytes()]);
}

struct Script {
    peer: Peer,
    replies: Mutex<std::collections::VecDeque<Result<Reply, TransportError>>>,
    seen: Mutex<Vec<Option<String>>>,
}

impl Script {
    fn new(peer: Peer, replies: impl IntoIterator<Item = Result<Reply, TransportError>>) -> Self {
        Self {
            peer,
            replies: Mutex::new(replies.into_iter().collect()),
            seen: Mutex::new(vec![]),
        }
    }
}

impl PullClient for Script {
    fn get<'a>(
        &'a self,
        _: &'a Publisher,
        target: &'a PathAndQuery,
        conditional: Option<&'a HeaderValue>,
    ) -> PullFuture<'a> {
        Box::pin(async move {
            assert_eq!(target.as_str(), GET);
            self.seen
                .lock()
                .unwrap()
                .push(conditional.map(|value| value.to_str().unwrap().into()));
            let reply = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("tentativa fora do orçamento")?;
            Ok(PullResponse {
                peer: self.peer.clone(),
                status: reply.status,
                headers: reply.headers,
                body: reply.body,
            })
        })
    }
}

struct Scenario {
    receiver: Arc<NodeIdentity>,
    issuer: Arc<NodeIdentity>,
    issuer_db: PgPool,
    receiver_db: PgPool,
    receiver_portable: Arc<PortableJoin>,
    issuer_plane: Arc<ConfigPlane>,
    receiver_plane: Arc<ConfigPlane>,
    receiver_settings: ConfigPlaneConfig,
    voters: Vec<NodeIdentity>,
}

impl Scenario {
    async fn new(issuer_db: PgPool, receiver_db: PgPool, url: &str) -> Self {
        let issuer = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let receiver = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let voters: Vec<_> = (0..3)
            .map(|_| NodeIdentity::generate_in_memory().unwrap())
            .collect();
        let issuer_portable = portable(issuer_db.clone(), issuer.clone(), &[&issuer, &receiver]);
        let receiver_portable =
            portable(receiver_db.clone(), receiver.clone(), &[&issuer, &receiver]);
        issuer_portable
            .import_genesis(
                &receiver_portable.local_manifest().unwrap(),
                Default::default(),
            )
            .await
            .unwrap();
        receiver_portable
            .import_genesis(
                &issuer_portable.local_manifest().unwrap(),
                Default::default(),
            )
            .await
            .unwrap();
        let make = |role| {
            let cfg = env(role, &voters, &[&issuer, &receiver], url);
            ConfigPlaneConfig::from_env_with(|key| cfg.get(key).cloned())
                .unwrap()
                .unwrap()
        };
        let issuer_plane = plane(issuer_portable, issuer.clone(), make(Role::Publisher)).await;
        let receiver_settings = make(Role::Receiver);
        let receiver_plane = plane(
            receiver_portable.clone(),
            receiver.clone(),
            receiver_settings.clone(),
        )
        .await;
        Self {
            receiver,
            issuer,
            issuer_db,
            receiver_db,
            receiver_portable,
            issuer_plane,
            receiver_plane,
            receiver_settings,
            voters,
        }
    }
}

fn data_reply(body: Vec<u8>, hash: &str) -> Reply {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{hash}\"")).unwrap(),
    );
    Reply {
        status: StatusCode::OK,
        headers,
        body,
    }
}

#[tokio::test]
async fn pull_503_do_servidor_usa_orcamento_e_preserva_efetivo() {
    let Some(a) = database("errors_a").await else {
        return;
    };
    let b = database("errors_b").await.unwrap();
    let s = Scenario::new(a, b, "https://127.0.0.1").await;
    let (addr, server) = server(&s.issuer_plane, Some(peer(&s.issuer))).await;
    let (frame, hash) = signed(&s.voters, 1, 1, 1);
    let (conflict, alternative_hash) = signed(&s.voters, 1, 2, 1);
    let admin = [("Authorization", "Bearer config-admin-test")];
    assert_eq!(call(addr, "PUT", ROUTE, &admin, &frame).await.status, 201);
    let data = call(addr, "GET", GET, &[], &[]).await;
    assert_eq!(
        s.receiver_plane
            .poll_cycle(&Script::new(peer(&s.issuer), [Ok(data)]))
            .await
            .unwrap(),
        PullResult::Written(WriteOutcome::Accepted)
    );
    s.receiver_plane
        .state()
        .activate_due(&s.receiver_portable, || 100)
        .await
        .unwrap();
    let before = snapshot(&s.receiver_db).await;
    assert_eq!(
        call(addr, "PUT", ROUTE, &admin, &conflict).await.status,
        409
    );
    let remote_conflict = call(addr, "GET", GET, &[], &[]).await;
    assert_eq!(remote_conflict.error(), "config_conflict");
    let script = Script::new(peer(&s.issuer), [Ok(remote_conflict.clone())]);
    assert_eq!(
        s.receiver_plane.poll_cycle(&script).await,
        Err(PullError::RemoteConflict)
    );
    assert_eq!(*script.seen.lock().unwrap(), [Some(format!("\"{hash}\""))]);
    assert_eq!(snapshot(&s.receiver_db).await, before);
    for (now, expected) in [(199, true), (200, false)] {
        let mut tx = s.receiver_db.begin().await.unwrap();
        let mut guard = s
            .receiver_portable
            .lock_cluster_guard(&mut tx)
            .await
            .unwrap();
        let fence = s
            .receiver_plane
            .state()
            .classify_fence_in_tx(&mut guard, 1, now)
            .await
            .unwrap();
        assert_eq!(
            matches!(fence, godmesh_server::config_bundle::Fence::Allowed { .. }),
            expected
        );
        tx.commit().await.unwrap();
    }

    s.issuer_db.close().await;
    let unavailable = call(addr, "GET", GET, &[], &[]).await;
    assert_eq!(unavailable.error(), "storage_unavailable");
    let script = Script::new(
        peer(&s.issuer),
        [
            Ok(unavailable.clone()),
            Err(TransportError::Network),
            Ok(unavailable.clone()),
        ],
    );
    assert_eq!(
        s.receiver_plane.poll_cycle(&script).await,
        Err(PullError::RemoteStorage)
    );
    assert_eq!(script.seen.lock().unwrap().len(), 3);
    for response in [unavailable, remote_conflict] {
        let mut parsed: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
        let duplicate = String::from_utf8(response.body.clone()).unwrap();
        let duplicate =
            format!("{{\"error\":\"storage_unavailable\",{}", &duplicate[1..]).into_bytes();
        let code = parsed.as_object_mut().unwrap().remove("error").unwrap();
        let missing = serde_json::to_vec(&parsed).unwrap();
        parsed["code"] = code;
        let only_code = serde_json::to_vec(&parsed).unwrap();
        for body in [
            vec![],
            duplicate,
            missing,
            only_code,
            response.body[..response.body.len() - 1].to_vec(),
        ] {
            let script = Script::new(
                peer(&s.issuer),
                [Ok(Reply {
                    body,
                    ..response.clone()
                })],
            );
            assert_eq!(
                s.receiver_plane.poll_cycle(&script).await,
                Err(PullError::InvalidErrorBody)
            );
            assert_eq!(script.seen.lock().unwrap().len(), 1);
        }
        let mut bad = response;
        bad.headers
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/plain"));
        let script = Script::new(peer(&s.issuer), [Ok(bad)]);
        assert_eq!(
            s.receiver_plane.poll_cycle(&script).await,
            Err(PullError::InvalidErrorBody)
        );
    }
    assert_eq!(snapshot(&s.receiver_db).await, before);
    let alternative = data_reply(conflict, &alternative_hash);
    assert_eq!(
        s.receiver_plane
            .poll_cycle(&Script::new(peer(&s.issuer), [Ok(alternative)]))
            .await
            .unwrap(),
        PullResult::Written(WriteOutcome::ConflictRecorded)
    );
    assert_eq!(snapshot(&s.receiver_db).await, (2, 3));
    let (receiver_addr, receiver_app) =
        crate::server(&s.receiver_plane, Some(peer(&s.receiver))).await;
    let (replacement, digest) = signed(&s.voters, 3, 1, 1);
    let path = format!("{ROUTE}?resolve_conflict=true&expected_hash={digest}");
    assert_eq!(call(receiver_addr, "GET", GET, &[], &[]).await.status, 403);
    assert_eq!(
        call(receiver_addr, "PUT", ROUTE, &admin, &replacement)
            .await
            .status,
        403
    );
    assert_eq!(
        call(receiver_addr, "PUT", &path, &admin, &replacement)
            .await
            .status,
        201
    );
    let published: Option<String> =
        sqlx::query_scalar("SELECT published FROM federation_config_state")
            .fetch_one(&s.receiver_db)
            .await
            .unwrap();
    assert!(published.is_none());
    receiver_app.abort();
    server.abort();
}

#[tokio::test]
async fn pull_real_mtls_confere_304_sem_alterar_estado() {
    use godmesh_server::config_https::{HttpsClient, serve};
    let Some(a) = database("tls_a").await else {
        return;
    };
    let b = database("tls_b").await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let s = Scenario::new(a, b, &format!("https://{endpoint}")).await;
    let (addr, app) = server(&s.issuer_plane, Some(peer(&s.issuer))).await;
    let (frame, _) = signed(&s.voters, 1, 1, 1);
    assert_eq!(
        call(
            addr,
            "PUT",
            ROUTE,
            &[("Authorization", "Bearer config-admin-test")],
            &frame
        )
        .await
        .status,
        201
    );
    app.abort();
    // O receptor autentica a chave do publicador e o publicador a chave do receptor.
    let settings = &s.receiver_settings;
    let (stop, shutdown) = tokio::sync::watch::channel(false);
    let limits = settings.https_limits();
    let router = s.issuer_plane.router();
    let identity = s.issuer.clone();
    let subject = s.receiver_portable.local_manifest().unwrap();
    let key: [u8; 32] = hex::decode(subject.public_key).unwrap().try_into().unwrap();
    let worker = tokio::spawn(async move {
        serve(listener, router, &identity, vec![key], limits, shutdown)
            .await
            .unwrap()
    });

    let client = HttpsClient::new(&s.receiver, limits).unwrap();
    assert_eq!(
        s.receiver_plane.poll_cycle(&client).await.unwrap(),
        PullResult::Written(WriteOutcome::Accepted)
    );
    let before = snapshot(&s.receiver_db).await;
    assert_eq!(
        s.receiver_plane.poll_cycle(&client).await.unwrap(),
        PullResult::NotModified
    );
    let restarted = plane(
        s.receiver_portable.clone(),
        s.receiver.clone(),
        settings.clone(),
    )
    .await;
    assert_eq!(
        restarted.poll_cycle(&client).await.unwrap(),
        PullResult::NotModified
    );
    assert_eq!(snapshot(&s.receiver_db).await, before);
    stop.send(true).unwrap();
    worker.await.unwrap();
}

struct Gated {
    publisher: Peer,
    response: OnceReply,
    started: tokio::sync::Notify,
    release: tokio::sync::Notify,
    in_flight: AtomicUsize,
    calls: AtomicUsize,
}

type OnceReply = Mutex<Option<Reply>>;

impl Gated {
    fn new(publisher: Peer, response: Reply) -> Self {
        Self {
            publisher,
            response: Mutex::new(Some(response)),
            started: Default::default(),
            release: Default::default(),
            in_flight: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
        }
    }
}

struct Attempt<'a>(&'a AtomicUsize);
impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl PullClient for Gated {
    fn get<'a>(
        &'a self,
        _: &'a Publisher,
        _: &'a PathAndQuery,
        _: Option<&'a HeaderValue>,
    ) -> PullFuture<'a> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(self.in_flight.fetch_add(1, Ordering::SeqCst), 0);
            let _attempt = Attempt(&self.in_flight);
            self.started.notify_one();
            self.release.notified().await;
            let reply = self
                .response
                .lock()
                .unwrap()
                .take()
                .expect("resposta única");
            Ok(PullResponse {
                peer: self.publisher.clone(),
                status: reply.status,
                headers: reply.headers,
                body: reply.body,
            })
        })
    }
}

#[tokio::test]
async fn pull_cancelamento_aborta_rede_e_libera_o_ciclo() {
    let Some(a) = database("cancel_a").await else {
        return;
    };
    let b = database("cancel_b").await.unwrap();
    let s = Scenario::new(a, b, "https://127.0.0.1").await;
    let (frame, hash) = signed(&s.voters, 1, 1, 1);
    let reply = data_reply(frame, &hash);
    let stalled = Arc::new(Gated::new(peer(&s.issuer), reply.clone()));
    let (stop, shutdown) = tokio::sync::watch::channel(false);
    let worker = {
        let plane = s.receiver_plane.clone();
        let stalled = stalled.clone();
        tokio::spawn(async move { plane.run_polling(stalled.as_ref(), shutdown).await })
    };
    tokio::time::timeout(Duration::from_secs(5), stalled.started.notified())
        .await
        .unwrap();
    let backup = Arc::new(Script::new(peer(&s.issuer), [Ok(reply)]));
    let queued = {
        let plane = s.receiver_plane.clone();
        let backup = backup.clone();
        tokio::spawn(async move { plane.poll_cycle(backup.as_ref()).await })
    };
    assert_eq!(stalled.in_flight.load(Ordering::SeqCst), 1);
    assert!(backup.seen.lock().unwrap().is_empty());
    assert_eq!(snapshot(&s.receiver_db).await, (0, 0));
    stop.send(true).unwrap();
    worker.await.unwrap();
    assert_eq!(stalled.in_flight.load(Ordering::SeqCst), 0);
    assert_eq!(stalled.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        queued.await.unwrap().unwrap(),
        PullResult::Written(WriteOutcome::Accepted)
    );
    assert_eq!(snapshot(&s.receiver_db).await, (1, 1));
}

async fn set_portable_peer(state: &str, pool: &PgPool, portal: &PortableJoin, node_id: &str) {
    let mut tx = pool.begin().await.unwrap();
    let mut guard = portal.lock_cluster_guard(&mut tx).await.unwrap();
    sqlx::query("UPDATE federation_peer SET trust_state = $1 WHERE node_id = $2")
        .bind(state)
        .bind(node_id)
        .execute(guard.connection())
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn pull_revalida_publicador_apos_rede_antes_de_evidencia() {
    let Some(a) = database("revoke_a").await else {
        return;
    };
    let b = database("revoke_b").await.unwrap();
    let s = Scenario::new(a, b, "https://127.0.0.1").await;
    let (frame, hash) = signed(&s.voters, 1, 1, 1);
    assert_eq!(
        s.receiver_plane
            .poll_cycle(&Script::new(
                peer(&s.issuer),
                [Ok(data_reply(frame, &hash))]
            ))
            .await
            .unwrap(),
        PullResult::Written(WriteOutcome::Accepted)
    );
    let before = snapshot(&s.receiver_db).await;
    let mut changes = s.receiver_plane.changes();
    for (version, floor, software) in [(1, 2, 1), (2, 2, 2)] {
        set_portable_peer(
            "authorized",
            &s.receiver_db,
            &s.receiver_portable,
            s.issuer.node_id(),
        )
        .await;
        let (candidate, digest) = signed(&s.voters, version, floor, software);
        let gated = Arc::new(Gated::new(peer(&s.issuer), data_reply(candidate, &digest)));
        let runner = {
            let plane = s.receiver_plane.clone();
            let gated = gated.clone();
            tokio::spawn(async move { plane.poll_cycle(gated.as_ref()).await })
        };
        tokio::time::timeout(Duration::from_secs(5), gated.started.notified())
            .await
            .unwrap();
        set_portable_peer(
            "revoked",
            &s.receiver_db,
            &s.receiver_portable,
            s.issuer.node_id(),
        )
        .await;
        gated.release.notify_one();
        let result = runner.await.unwrap().unwrap();
        assert_eq!(result, PullResult::Denied(PullDenial::Authority));
        assert_eq!(result.write_outcome(), None);
        assert_eq!(snapshot(&s.receiver_db).await, before);
        assert!(!changes.has_changed().unwrap());
        changes.borrow_and_update();
    }
}

async fn wait_blocked(pool: &PgPool, kind: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_locks
              WHERE NOT granted AND locktype = $1 AND database =
                (SELECT oid FROM pg_database WHERE datname = current_database()))",
            )
            .bind(kind)
            .fetch_one(pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn pull_primeiro_no_lock_confirma_antes_da_revogacao() {
    let Some(a) = database("first_a").await else {
        return;
    };
    let b = database("first_b").await.unwrap();
    let s = Scenario::new(a, b, "https://127.0.0.1").await;
    let (frame, hash) = signed(&s.voters, 1, 1, 1);
    let gate = Arc::new(Gated::new(peer(&s.issuer), data_reply(frame, &hash)));
    let pulling = {
        let (plane, gate) = (s.receiver_plane.clone(), gate.clone());
        tokio::spawn(async move { plane.poll_cycle(gate.as_ref()).await })
    };
    tokio::time::timeout(Duration::from_secs(5), gate.started.notified())
        .await
        .unwrap();
    let mut barrier = s.receiver_db.begin().await.unwrap();
    sqlx::query("LOCK TABLE federation_config_state IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *barrier)
        .await
        .unwrap();
    gate.release.notify_one();
    wait_blocked(&s.receiver_db, "relation").await;
    let revoking = {
        let db = s.receiver_db.clone();
        let portable = s.receiver_portable.clone();
        let issuer = s.issuer.clone();
        tokio::spawn(async move {
            set_portable_peer("revoked", &db, &portable, issuer.node_id()).await;
        })
    };
    wait_blocked(&s.receiver_db, "advisory").await;
    assert!(!revoking.is_finished());
    barrier.commit().await.unwrap();
    assert_eq!(
        pulling.await.unwrap().unwrap(),
        PullResult::Written(WriteOutcome::Accepted)
    );
    revoking.await.unwrap();
    assert_eq!(snapshot(&s.receiver_db).await, (1, 1));
    let (new, digest) = signed(&s.voters, 2, 1, 1);
    assert_eq!(
        s.receiver_plane
            .poll_cycle(&Script::new(
                peer(&s.issuer),
                [Ok(data_reply(new, &digest))]
            ))
            .await
            .unwrap(),
        PullResult::Denied(PullDenial::Authority)
    );
    assert_eq!(snapshot(&s.receiver_db).await, (1, 1));
}

#[tokio::test]
async fn put_conta_bytes_reais_no_corpo_chunked() {
    let Some(pool) = database("body").await else {
        return;
    };
    let who = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let voters: Vec<_> = (0..3)
        .map(|_| NodeIdentity::generate_in_memory().unwrap())
        .collect();
    let cfg = env(Role::Publisher, &voters, &[&who], "");
    let cfg = ConfigPlaneConfig::from_env_with(|key| cfg.get(key).cloned())
        .unwrap()
        .unwrap();
    let limit = cfg.max_body_bytes.get();
    let app = plane(
        portable(pool.clone(), who.clone(), &[&who]),
        who.clone(),
        cfg,
    )
    .await;
    let (addr, task) = server(&app, Some(peer(&who))).await;
    let (mut frame, _) = signed(&voters, 1, 1, 1);
    frame.resize(limit, b' ');
    let wire = |frame: &[u8]| {
        let mut req=format!("PUT {ROUTE} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nAuthorization: Bearer config-admin-test\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",frame.len()).into_bytes();
        req.extend_from_slice(frame);
        req.extend_from_slice(b"\r\n0\r\n\r\n");
        req
    };
    assert_eq!(raw_call(addr, wire(&frame)).await.status, 201);
    let before = snapshot(&pool).await;
    frame.push(b' ');
    let rejected = raw_call(addr, wire(&frame)).await;
    assert_eq!(rejected.status, 413);
    assert_eq!(rejected.error(), "payload_too_large");
    assert_eq!(snapshot(&pool).await, before);
    task.abort();
}

#[tokio::test]
async fn api_serializa_etag_publicacao_e_resolucao_com_revogacao() {
    for (suffix, operation) in [("etag", 0), ("publish", 1), ("resolve", 2)] {
        let Some(a) = database(&format!("api_{suffix}_a")).await else {
            return;
        };
        let b = database(&format!("api_{suffix}_b")).await.unwrap();
        let s = Scenario::new(a, b, "https://127.0.0.1").await;
        let a_portable = portable(
            s.issuer_db.clone(),
            s.issuer.clone(),
            &[&s.issuer, &s.receiver],
        );
        let (seed, local_app) = server(&s.issuer_plane, Some(peer(&s.issuer))).await;
        let (v1, digest) = signed(&s.voters, 1, 1, 1);
        let (v2, v2_hash) = signed(&s.voters, 2, 2, 1);
        let admin = [("Authorization", "Bearer config-admin-test")];
        assert_eq!(call(seed, "PUT", ROUTE, &admin, &v1).await.status, 201);
        if operation == 2 {
            let (other, _) = signed(&s.voters, 1, 2, 1);
            assert_eq!(call(seed, "PUT", ROUTE, &admin, &other).await.status, 409);
        }
        local_app.abort();
        let (addr, app) = server(&s.issuer_plane, Some(peer(&s.receiver))).await;
        let path = match operation {
            0 => GET.to_owned(),
            1 => ROUTE.to_owned(),
            _ => format!("{ROUTE}?resolve_conflict=true&expected_hash={v2_hash}"),
        };
        let tag = format!("\"{digest}\"");
        let mut barrier = s.issuer_db.begin().await.unwrap();
        sqlx::query("LOCK TABLE federation_config_state IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *barrier)
            .await
            .unwrap();
        let request = {
            let (path, tag, v2) = (path.clone(), tag.clone(), v2.clone());
            tokio::spawn(async move {
                if operation == 0 {
                    call(addr, "GET", &path, &[("If-None-Match", &tag)], &[]).await
                } else {
                    call(addr, "PUT", &path, &admin, &v2).await
                }
            })
        };
        wait_blocked(&s.issuer_db, "relation").await;
        let revoking = {
            let db = s.issuer_db.clone();
            let receiver = s.receiver.clone();
            tokio::spawn(async move {
                set_portable_peer("revoked", &db, &a_portable, receiver.node_id()).await
            })
        };
        wait_blocked(&s.issuer_db, "advisory").await;
        barrier.commit().await.unwrap();
        assert_eq!(
            request.await.unwrap().status,
            if operation == 0 { 304 } else { 201 }
        );
        revoking.await.unwrap();
        let before = snapshot(&s.issuer_db).await;
        let rejected = if operation == 0 {
            call(addr, "GET", &path, &[("If-None-Match", &tag)], &[]).await
        } else {
            call(addr, "PUT", &path, &admin, &v2).await
        };
        assert_eq!(rejected.status, 403);
        assert_eq!(snapshot(&s.issuer_db).await, before);
        app.abort();
    }
}

#[tokio::test]
async fn pull_prazo_no_postgres_condicional_e_aceitacao() {
    let Some(a) = database("deadline_a").await else {
        return;
    };
    let b = database("deadline_b").await.unwrap();
    let s = Scenario::new(a, b, "https://127.0.0.1").await;
    let mut cfg = s.receiver_settings.clone();
    cfg.pull_timeout = Duration::from_millis(300);
    cfg.max_attempts = std::num::NonZeroUsize::MIN;
    let plane = plane(s.receiver_portable.clone(), s.receiver.clone(), cfg).await;
    let (frame, hash) = signed(&s.voters, 1, 1, 1);
    let reply = data_reply(frame, &hash);
    let no_network = Script::new(peer(&s.issuer), []);
    let mut barrier = s.receiver_db.begin().await.unwrap();
    s.receiver_portable
        .lock_cluster_guard(&mut barrier)
        .await
        .unwrap();
    let conditional =
        tokio::time::timeout(Duration::from_secs(2), plane.poll_cycle(&no_network)).await;
    barrier.rollback().await.unwrap();
    assert_eq!(
        conditional.expect("leitura condicional deve respeitar o prazo"),
        Err(PullError::Storage {
            effect_unknown: false
        })
    );
    assert!(no_network.seen.lock().unwrap().is_empty());

    let gated = Arc::new(Gated::new(peer(&s.issuer), reply.clone()));
    let runner = {
        let (plane, gated) = (plane.clone(), gated.clone());
        tokio::spawn(async move { plane.poll_cycle(gated.as_ref()).await })
    };
    tokio::time::timeout(Duration::from_secs(2), gated.started.notified())
        .await
        .unwrap();
    let mut barrier = s.receiver_db.begin().await.unwrap();
    s.receiver_portable
        .lock_cluster_guard(&mut barrier)
        .await
        .unwrap();
    gated.release.notify_one();
    let accepting = tokio::time::timeout(Duration::from_secs(2), runner).await;
    barrier.rollback().await.unwrap();
    assert_eq!(
        accepting
            .expect("aceitação deve respeitar o prazo")
            .unwrap(),
        Err(PullError::Storage {
            effect_unknown: false
        })
    );
    assert_eq!(snapshot(&s.receiver_db).await, (0, 0));
    let resumed = Script::new(peer(&s.issuer), [Ok(reply)]);
    assert_eq!(
        plane.poll_cycle(&resumed).await.unwrap(),
        PullResult::Written(WriteOutcome::Accepted)
    );
}

#[tokio::test]
async fn pull_commit_lento_tem_efeito_desconhecido_e_notifica_apos_confirmacao() {
    let Some(a) = database("commit_deadline_a").await else {
        return;
    };
    let b = database("commit_deadline_b").await.unwrap();
    let s = Scenario::new(a, b, "https://127.0.0.1").await;
    let mut cfg = s.receiver_settings.clone();
    cfg.pull_timeout = Duration::from_millis(500);
    let plane = plane(s.receiver_portable.clone(), s.receiver.clone(), cfg).await;
    sqlx::raw_sql(
        "CREATE FUNCTION delay_pull_commit() RETURNS trigger LANGUAGE plpgsql AS
        'BEGIN PERFORM pg_advisory_xact_lock(81421); RETURN NEW; END';
        CREATE CONSTRAINT TRIGGER delay_pull_commit AFTER INSERT ON federation_config_audit
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION delay_pull_commit();",
    )
    .execute(&s.receiver_db)
    .await
    .unwrap();
    let mut barrier = s.receiver_db.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(81421)")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let (frame, hash) = signed(&s.voters, 1, 1, 1);
    let client = Script::new(peer(&s.issuer), [Ok(data_reply(frame, &hash))]);
    let mut changes = plane.changes();
    let result = tokio::time::timeout(Duration::from_secs(2), plane.poll_cycle(&client)).await;
    assert_eq!(snapshot(&s.receiver_db).await, (0, 0));
    assert!(!changes.has_changed().unwrap());
    barrier.rollback().await.unwrap();
    assert_eq!(
        result.expect("COMMIT deve respeitar o prazo"),
        Err(PullError::Storage {
            effect_unknown: true
        })
    );
    tokio::time::timeout(Duration::from_secs(2), changes.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot(&s.receiver_db).await, (1, 1));
    assert_eq!(
        client.seen.lock().unwrap().len(),
        1,
        "efeito desconhecido não entra em retry"
    );
}
