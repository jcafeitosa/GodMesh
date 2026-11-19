use super::server;
use godmesh_federation::{
    NodeIdentity,
    config_bundle::{
        BundleBody, BundleKey, BundlePolicy, PinnedVoter, SoftwareVersion, vote_bytes,
    },
    event::{EventDraft, signed_event},
};
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use serde_json::json;
use server::{
    auth::{Scope, Subject, VerifiedGrant},
    config_bundle::{BundleService, Preflight, Role as BundleRole, ServicePolicy},
    federation::JoinWindow,
    portable_join::{PortableJoin, PortableJoinConfig},
    replica_operational::{PreflightError, ReplicaConfig, ReplicaService, Role},
    replication::{
        Limits, NodeContext, OriginAdmission, PolicyRef, Replication, Reservation, Series,
    },
};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

pub fn env(role: &str) -> Vec<(String, String)> {
    let key = NodeIdentity::generate_in_memory().unwrap();
    let pin = format!("{}:{}", key.node_id(), key.public_key_hex());
    [
        ("ROLE", role.to_owned()),
        ("CLUSTER", "fixture".to_owned()),
        ("TENANT_ID", "tenant".to_owned()),
        ("STREAM_ID", "m4a.metadata.fixture".to_owned()),
        ("PARTITION", "0".to_owned()),
        ("AUTHORITY_EPOCH", i64::MAX.to_string()),
        ("ORIGIN_PIN", pin),
        ("MAX_FRAME_BYTES", "4096".to_owned()),
        ("MAX_ENVELOPE_BYTES", "2048".to_owned()),
        ("MAX_EVENTS", "16".to_owned()),
        ("MAX_STREAMS_PER_CONNECTION", "1".to_owned()),
        ("MAX_ATTEMPTS", "2".to_owned()),
        ("ATTEMPT_TIMEOUT_MS", "1000".to_owned()),
        ("RETRY_INTERVAL_MS", "10".to_owned()),
        ("ACL_RECHECK_INTERVAL_MS", "10".to_owned()),
        ("PULL_INTERVAL_MS", "10".to_owned()),
    ]
    .into_iter()
    .map(|(name, value)| (format!("GODMESH_FEDERATION_M4_{name}"), value))
    .collect()
}

pub const CLUSTER: &str = "m4-runtime-fixture";
pub const CLASS: &str = "m4_runtime_fixture";

pub async fn isolated_database(label: &str) -> Option<sqlx::PgPool> {
    let base = std::env::var("GODMESH_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?
        .parse::<sqlx::postgres::PgConnectOptions>()
        .unwrap();
    let admin = sqlx::PgPool::connect_with(base.clone()).await.unwrap();
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(version.parse::<u32>().unwrap() / 10_000, 16);
    let database = format!("m4a2_{}_{label}", std::process::id());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let pool = sqlx::PgPool::connect_with(base.database(&database))
        .await
        .unwrap();
    server::migrate::migrate(
        &pool,
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations"),
    )
    .await
    .unwrap();
    Some(pool)
}

pub struct ReceiverFixture {
    pub pool: sqlx::PgPool,
    pub config: ReplicaConfig,
    pub portable: Arc<PortableJoin>,
    #[allow(dead_code)] // Usada nos testes de conexão A2.
    pub portable_config: PortableJoinConfig,
    pub bundle: Arc<BundleService>,
    pub origin: Arc<NodeIdentity>,
    pub receiver: Arc<NodeIdentity>,
}

impl ReceiverFixture {
    pub async fn new(label: &str) -> Option<Self> {
        let origin = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let receiver = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let tenant = receiver.node_id().to_owned();
        Self::new_with(label, origin, receiver, &tenant).await
    }

    pub async fn new_with(
        label: &str,
        origin: Arc<NodeIdentity>,
        receiver: Arc<NodeIdentity>,
        tenant: &str,
    ) -> Option<Self> {
        let pool = isolated_database(label).await?;
        let pin = |id: &NodeIdentity| format!("{}:{}", id.node_id(), id.public_key_hex());
        let pins = format!("{};{}", pin(&origin), pin(&receiver));
        let portable_config = configured_portable(&pins);
        let source = PortableJoin::new(portable_config.clone(), pool.clone(), origin.clone());
        let portable = Arc::new(PortableJoin::new(
            portable_config.clone(),
            pool.clone(),
            receiver.clone(),
        ));
        portable
            .import_genesis(&source.local_manifest().unwrap(), JoinWindow::default())
            .await
            .unwrap();

        let voters: Vec<_> = (0..3)
            .map(|_| NodeIdentity::generate_in_memory().unwrap())
            .collect();
        let software = SoftwareVersion::new(1, 0, 0).unwrap();
        let policy = ServicePolicy {
            profile: 1,
            role: BundleRole::Receiver,
            software,
            bundle: BundlePolicy {
                cluster: CLUSTER.to_owned(),
                epoch: 7,
                max_frame_bytes: 8192.try_into().unwrap(),
                voters: voters
                    .iter()
                    .map(|id| PinnedVoter {
                        node_id: id.node_id().to_owned(),
                        public_key: id.public_key_hex(),
                    })
                    .collect(),
            },
        };
        let Preflight::Enabled(bundle) =
            BundleService::preflight(&pool, Some(&portable), Some(policy))
                .await
                .unwrap()
        else {
            panic!("M3")
        };
        let body = BundleBody {
            key: BundleKey {
                cluster: CLUSTER.into(),
                namespace: "federation".into(),
                scope_id: String::new(),
                epoch: 7,
            },
            version: 1,
            applies_at: 100,
            expires_at: 500,
            required_signatures: 3,
            value: 1,
            effective_after: 100,
            min_version: software,
        };
        let digest = body.digest();
        let signatures: Vec<_> = voters
            .iter()
            .map(|id| {
                json!({
                    "signer_node_id": id.node_id(), "signature": id.sign_hex(&vote_bytes(&digest)),
                })
            })
            .collect();
        let raw = serde_json::to_vec(&json!({
            "profile": 1, "cluster": CLUSTER, "namespace": "federation", "scope_id": "", "epoch": 7,
            "version": 1, "applies_at": 100, "expires_at": 500, "required_signatures": 3,
            "entries": [{"key":"federation.minimum_protocol_version","value":1,"effective_after":100,"min_version":"1.0.0"}],
            "hash": hex::encode(digest), "signatures": signatures,
        })).unwrap();
        bundle.accept(&portable, &raw, || 100).await.unwrap();
        bundle.activate_due(&portable, || 100).await.unwrap();
        let config =
            ReplicaConfig::from_env_with(env("receiver").into_iter().map(|(key, value)| {
                let value = match key.strip_prefix("GODMESH_FEDERATION_M4_").unwrap() {
                    "CLUSTER" => CLUSTER.to_owned(),
                    "ORIGIN_PIN" => pin(&origin),
                    "TENANT_ID" => tenant.to_owned(),
                    "AUTHORITY_EPOCH" => "7".to_owned(),
                    _ => value,
                };
                (key, value)
            }))
            .unwrap()
            .unwrap();
        let tenant = &config.namespace.tenant_id;
        sqlx::query("INSERT INTO tenant_region (tenant_id,jurisdiction) VALUES ($1,'BR')")
            .bind(tenant)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO retention_class VALUES ($1,'BR','fixture',1,'expiry') ON CONFLICT DO NOTHING")
            .bind(CLASS).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO federation_replica_policy
            VALUES ($1,1,$2,$3,1,'BR',$1,'secondary','replica_metadata','issued_at',200,true,'explicit',true)")
            .bind(receiver.node_id()).bind(tenant).bind(CLASS).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO federation_replica_acl
            VALUES ($1,$2,$3,0,7,$4,'import',$5,1,'BR','secondary',$4,1,0,1000,false)",
        )
        .bind(origin.node_id())
        .bind(tenant)
        .bind(&config.namespace.stream_id)
        .bind(receiver.node_id())
        .bind(CLASS)
        .execute(&pool)
        .await
        .unwrap();
        Some(Self {
            pool,
            config,
            portable,
            portable_config,
            bundle: Arc::new(bundle),
            origin,
            receiver,
        })
    }

    pub async fn reserve(&self) {
        let mut tx = self.pool.begin().await.unwrap();
        let mut guard = self.portable.lock_cluster_guard(&mut tx).await.unwrap();
        Replication::reserve_namespace_in_tx(
            &mut guard,
            &Reservation {
                namespace: self.config.namespace.clone(),
                authority_epoch: 7,
                local_node: self.receiver.node_id().to_owned(),
                profile: "metadata".to_owned(),
                profile_version: 1,
                retention_class: CLASS.to_owned(),
                class_version: 1,
                jurisdiction: "BR".to_owned(),
                policy: PolicyRef {
                    id: self.receiver.node_id().to_owned(),
                    version: 1,
                },
            },
            NodeContext {
                node_id: self.receiver.node_id(),
                public_key: &self.receiver.public_key_hex(),
            },
            || 100,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }

    pub async fn boot(&self) -> Result<ReplicaService, PreflightError> {
        ReplicaService::preflight(
            self.config.clone(),
            self.portable.clone(),
            self.bundle.clone(),
            &self.receiver,
        )
        .await
        .map(|service| service.with_clock(Arc::new(|| 110)))
    }

    pub fn frame(&self, seq: u64) -> Vec<u8> {
        serde_json::to_vec(&signed_event(
            &self.origin,
            EventDraft {
                tenant_id: &self.config.namespace.tenant_id,
                stream_id: &self.config.namespace.stream_id,
                partition: 0,
                authority_epoch: 7,
                seq,
                retention_class: CLASS,
                issued_at: 100,
            },
        ))
        .unwrap()
    }
}

pub fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}

pub(crate) fn configured_portable(pins: &str) -> PortableJoinConfig {
    let governance = format!("single_approver:{CLUSTER}");
    PortableJoinConfig::from_env_with(|name| {
        Some(
            match name {
                "GODMESH_FEDERATION_GOVERNANCE" => &governance,
                "GODMESH_FEDERATION_GENESIS" => pins,
                "GODMESH_FEDERATION_REGION" => "fixture",
                "GODMESH_FEDERATION_ADVERTISE_QUIC" => "quic://127.0.0.1:4433",
                "GODMESH_FEDERATION_PORTABLE_MAX_DEPTH" => "4",
                "GODMESH_FEDERATION_PORTABLE_MAX_FRAME_BYTES" => "8192",
                "GODMESH_FEDERATION_PORTABLE_MAX_VALIDITY_SECONDS" => "60",
                _ => return None,
            }
            .to_owned(),
        )
    })
    .unwrap()
    .unwrap()
}

pub async fn boot_source(
    f: &ReceiverFixture,
    other: Arc<NodeIdentity>,
    frames: &[Vec<u8>],
) -> ReplicaService {
    let pins = [&*f.origin, &*f.receiver, &*other]
        .into_iter()
        .map(|id| format!("{}:{}", id.node_id(), id.public_key_hex()))
        .collect::<Vec<_>>()
        .join(";");
    let config_portable = configured_portable(&pins);
    let source = Arc::new(PortableJoin::new(
        config_portable.clone(),
        f.pool.clone(),
        f.origin.clone(),
    ));
    source
        .import_genesis(&f.portable.local_manifest().unwrap(), JoinWindow::default())
        .await
        .unwrap();
    let other_service = PortableJoin::new(config_portable, f.pool.clone(), other.clone());
    source
        .import_genesis(
            &other_service.local_manifest().unwrap(),
            JoinWindow::default(),
        )
        .await
        .unwrap();
    let raw_pins = format!(
        "{}:{},{}:{}",
        f.receiver.node_id(),
        f.receiver.public_key_hex(),
        other.node_id(),
        other.public_key_hex()
    );
    let mut config = f.config.clone();
    config.role = Role::Origin;
    config.pull_interval = None;
    let mut vars = env("origin");
    vars.retain(|(key, _)| !key.ends_with("_PULL_INTERVAL_MS"));
    vars.push((
        "GODMESH_FEDERATION_M4_DESTINATION_PINS".to_owned(),
        raw_pins,
    ));
    config.destination_pins = ReplicaConfig::from_env_with(vars)
        .unwrap()
        .unwrap()
        .destination_pins;
    let ns = &config.namespace;
    sqlx::query("INSERT INTO federation_replica_policy
        VALUES ($1,1,$2,$3,1,'BR',$1,'primary','replica_metadata','issued_at',200,true,'explicit',true)")
        .bind(f.origin.node_id()).bind(&ns.tenant_id).bind(CLASS).execute(&f.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO federation_replica_acl
        VALUES ($1,$2,$3,0,7,$4,'export',$5,1,'BR','secondary',$4,1,0,1000,false)",
    )
    .bind(f.origin.node_id())
    .bind(&ns.tenant_id)
    .bind(&ns.stream_id)
    .bind(f.receiver.node_id())
    .bind(CLASS)
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO federation_replica_policy
        VALUES ($1,1,$2,$3,1,'BR',$1,'secondary','replica_metadata','issued_at',200,true,'explicit',true)")
        .bind(other.node_id()).bind(&ns.tenant_id).bind(CLASS).execute(&f.pool).await.unwrap();
    sqlx::query(
        "INSERT INTO federation_replica_acl
        VALUES ($1,$2,$3,0,7,$4,'export',$5,1,'BR','secondary',$4,1,0,1000,false)",
    )
    .bind(f.origin.node_id())
    .bind(&ns.tenant_id)
    .bind(&ns.stream_id)
    .bind(other.node_id())
    .bind(CLASS)
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO federation_peer_scope (node_id,tenant_id,stream_id) VALUES ($1,$2,$3)",
    )
    .bind(f.origin.node_id())
    .bind(&ns.tenant_id)
    .bind(&ns.stream_id)
    .execute(&f.pool)
    .await
    .unwrap();
    let origin_key = f.origin.public_key_hex();
    let local = NodeContext {
        node_id: f.origin.node_id(),
        public_key: &origin_key,
    };
    let mut tx = f.pool.begin().await.unwrap();
    let mut guard = source.lock_cluster_guard(&mut tx).await.unwrap();
    Replication::reserve_namespace_in_tx(
        &mut guard,
        &Reservation {
            namespace: ns.clone(),
            authority_epoch: 7,
            local_node: f.origin.node_id().to_owned(),
            profile: "metadata".to_owned(),
            profile_version: 1,
            retention_class: CLASS.to_owned(),
            class_version: 1,
            jurisdiction: "BR".to_owned(),
            policy: PolicyRef {
                id: f.origin.node_id().to_owned(),
                version: 1,
            },
        },
        local,
        || 100,
    )
    .await
    .unwrap();
    admit_frames(&mut guard, ns, local, frames).await;
    tx.commit().await.unwrap();

    ReplicaService::preflight(config, source, f.bundle.clone(), &f.origin)
        .await
        .unwrap()
        .with_clock(Arc::new(|| 110))
}

pub async fn admit_frames(
    guard: &mut server::portable_join::ClusterGuard<'_, '_>,
    ns: &server::replication::Namespace,
    local: NodeContext<'_>,
    frames: &[Vec<u8>],
) {
    let grant = VerifiedGrant {
        subject: Subject {
            tenant_id: ns.tenant_id.clone(),
            principal: "fixture".into(),
        },
        token_id: "fixture".into(),
        scopes: vec![Scope::EventWrite],
        authority_epoch: 1,
        predicate: GrantPredicate {
            operation: OfflineOp::RecordLocalObservation,
            epoch_comparable: true,
            revocation: RevocationState::Absent,
            freshness: FreshnessSource::F3VerifiedOnline,
            retention_class_known: true,
            action_enumerated: true,
        },
    };
    let limits = Limits::new(8, 2048, 4096).unwrap();
    for actual in frames {
        Replication::admit_origin_in_tx(
            guard,
            Series {
                namespace: ns,
                epoch: 7,
            },
            OriginAdmission {
                origin: local,
                grant: &grant,
                max_age_seconds: 10,
                limits,
            },
            actual,
            || 100,
        )
        .await
        .unwrap();
    }
}
