mod common;

use godmesh_federation::NodeIdentity;
use godmesh_server::replica_operational::{ReplicaConfig, Role};

fn env(role: &str) -> Vec<(String, String)> {
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

#[test]
fn opt_in_fecha_em_configuracao_parcial_desconhecida_ou_invalida() {
    assert!(ReplicaConfig::from_env_with(Vec::new()).unwrap().is_none());
    for field in ["ROLE", "UNKNOWN"] {
        let vars = vec![(format!("GODMESH_FEDERATION_M4_{field}"), "".to_owned())];
        assert!(ReplicaConfig::from_env_with(vars).is_err());
    }
    let fixture = env("receiver");
    let config = ReplicaConfig::from_env_with(fixture.clone())
        .unwrap()
        .unwrap();
    assert_eq!(config.role, Role::Receiver);
    assert_eq!(config.authority_epoch, i64::MAX);
    for (field, invalid) in [
        ("PARTITION", "+0"),
        ("AUTHORITY_EPOCH", "9223372036854775808"),
        ("ORIGIN_PIN", "fixture"),
        ("CLUSTER", "  "),
        ("MAX_FRAME_BYTES", "100"),
        ("MAX_ENVELOPE_BYTES", "4097"),
        ("MAX_EVENTS", "0"),
        ("ATTEMPT_TIMEOUT_MS", "18446744073709551616"),
        ("DESTINATION_PINS", "invalid"),
        ("PULL_INTERVAL_MS", ""),
    ] {
        let name = format!("GODMESH_FEDERATION_M4_{field}");
        let mut changed = fixture.clone();
        changed.retain(|(key, _)| key != &name);
        changed.push((name, invalid.to_owned()));
        assert!(ReplicaConfig::from_env_with(changed).is_err(), "{field}");
    }
}

#[tokio::test]
async fn runner_aplica_suspensao_sem_seed() {
    let Some(pool) = isolated_database("clean").await else {
        return;
    };
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM federation_replica_resume_block")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

use std::{sync::Arc, time::Duration};

use godmesh_federation::{
    config_bundle::{
        BundleBody, BundleKey, BundlePolicy, PinnedVoter, SoftwareVersion, vote_bytes,
    },
    event::{EventDraft, signed_event},
};
use godmesh_server::{
    config_bundle::{BundleService, Preflight, Role as BundleRole, ServicePolicy},
    federation::JoinWindow,
    portable_join::{PortableJoin, PortableJoinConfig},
    replica_operational::{
        EffectError, PreflightError, ReceiverState, ReplicaService, SessionPeer,
    },
    replication::{
        Error as ReplicaError, Limits, NodeContext, PolicyRef, Replication, Reservation,
    },
};
use serde_json::json;
use tokio::time::Instant;

const CLUSTER: &str = "m4-runtime-fixture";
const CLASS: &str = "m4_runtime_fixture";

async fn isolated_database(label: &str) -> Option<sqlx::PgPool> {
    let base = common::database_url()?
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
    godmesh_server::migrate::migrate(&pool, &common::migrations_dir())
        .await
        .unwrap();
    Some(pool)
}

struct ReceiverFixture {
    pool: sqlx::PgPool,
    config: ReplicaConfig,
    portable: Arc<PortableJoin>,
    portable_config: PortableJoinConfig,
    bundle: Arc<BundleService>,
    origin: Arc<NodeIdentity>,
    receiver: Arc<NodeIdentity>,
}

impl ReceiverFixture {
    async fn new(label: &str) -> Option<Self> {
        let pool = isolated_database(label).await?;
        let origin = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let receiver = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let pin = |id: &NodeIdentity| format!("{}:{}", id.node_id(), id.public_key_hex());
        let pins = format!("{};{}", pin(&origin), pin(&receiver));
        let governance = format!("single_approver:{CLUSTER}");
        let portable_config = PortableJoinConfig::from_env_with(|name| {
            Some(
                match name {
                    "GODMESH_FEDERATION_GOVERNANCE" => &governance,
                    "GODMESH_FEDERATION_GENESIS" => &pins,
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
        .unwrap();
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
                    "TENANT_ID" => receiver.node_id().to_owned(),
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

    async fn reserve(&self) {
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

    async fn boot(&self) -> Result<ReplicaService, PreflightError> {
        ReplicaService::preflight(
            self.config.clone(),
            self.portable.clone(),
            self.bundle.clone(),
            &self.receiver,
        )
        .await
        .map(|service| service.with_clock(Arc::new(|| 110)))
    }

    fn frame(&self, seq: u64) -> Vec<u8> {
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

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}

#[tokio::test]
async fn preflight_checkpoint_e_suspensao_correlacionada_sobrevivem_reabertura() {
    let Some(f) = ReceiverFixture::new("resume").await else {
        return;
    };
    assert!(matches!(f.boot().await, Err(PreflightError::Mismatch)));
    f.reserve().await;
    let service = f.boot().await.unwrap();
    let origin_key = f.origin.public_key_hex();
    let peer = SessionPeer {
        node_id: f.origin.node_id(),
        public_key: &origin_key,
        protocol: 1,
    };
    assert_eq!(
        service.checkpoint(peer, deadline()).await.unwrap(),
        ReceiverState::Ready { after_seq: 0 }
    );
    let low_protocol = SessionPeer {
        protocol: 0,
        ..peer
    };
    assert!(matches!(
        service.suspend_history(low_protocol, 0, deadline()).await,
        Err(EffectError::Fence)
    ));
    assert_eq!(
        service.checkpoint(peer, deadline()).await.unwrap(),
        ReceiverState::Ready { after_seq: 0 }
    );
    let limits = Limits::new(8, 2048, 4096).unwrap();
    let applied = service
        .apply_copy(peer, limits, &[f.frame(1)], deadline())
        .await
        .unwrap();
    assert_eq!(applied.prefix, 1);
    assert!(matches!(
        service.suspend_history(peer, 0, deadline()).await,
        Err(EffectError::StalePull)
    ));
    assert_eq!(
        service.checkpoint(peer, deadline()).await.unwrap(),
        ReceiverState::Ready { after_seq: 1 }
    );
    let blocked = ReceiverState::SuspendedHistory {
        blocked_next_seq: 2,
    };
    assert_eq!(
        service.suspend_history(peer, 1, deadline()).await.unwrap(),
        blocked
    );
    drop(service);
    let reopened = f.boot().await.unwrap();
    assert_eq!(
        reopened.checkpoint(peer, deadline()).await.unwrap(),
        blocked
    );
    assert_eq!(
        reopened.suspend_history(peer, 1, deadline()).await.unwrap(),
        blocked
    );
    assert!(matches!(
        reopened
            .apply_copy(peer, limits, &[f.frame(2)], deadline())
            .await,
        Err(EffectError::SuspendedHistory {
            blocked_next_seq: 2
        })
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM federation_replica_proof WHERE origin_node=$1")
            .bind(f.origin.node_id())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    sqlx::query("DELETE FROM federation_replica_checkpoint WHERE local_node=$1")
        .bind(f.receiver.node_id())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        reopened.checkpoint(peer, deadline()).await,
        Err(EffectError::Replication(ReplicaError::CheckpointMissing))
    ));
}

#[tokio::test]
async fn relogio_amostra_depois_do_namespace_e_expiracao_bloqueia_o_efeito() {
    use std::sync::atomic::{AtomicI64, Ordering};
    let Some(f) = ReceiverFixture::new("clock").await else {
        return;
    };
    f.reserve().await;
    let now = Arc::new(AtomicI64::new(110));
    let clock = now.clone();
    let service = f
        .boot()
        .await
        .unwrap()
        .with_clock(Arc::new(move || clock.load(Ordering::SeqCst)));
    let mut external = f.pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *external)
        .await
        .unwrap();
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended(
            jsonb_build_array('godmesh.event_position'::text, $1::text, $2::text,
                              $3::text, 0::int)::text, 0))",
    )
    .bind(&f.config.namespace.origin_node)
    .bind(&f.config.namespace.tenant_id)
    .bind(&f.config.namespace.stream_id)
    .execute(&mut *external)
    .await
    .unwrap();
    let origin = f.origin.clone();
    let pending = tokio::spawn(async move {
        let key = origin.public_key_hex();
        let peer = SessionPeer {
            node_id: origin.node_id(),
            public_key: &key,
            protocol: 1,
        };
        service.suspend_history(peer, 0, deadline()).await
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS (
                SELECT FROM pg_locks WHERE locktype='advisory' AND NOT granted AND $1 = ANY(pg_blocking_pids(pid))
            )").bind(pid).fetch_one(&f.pool).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    now.store(500, Ordering::SeqCst);
    external.commit().await.unwrap();
    assert!(matches!(pending.await.unwrap(), Err(EffectError::Fence)));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM federation_replica_resume_block")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn sql_preserva_teto_menor_e_retorna_sessao_limpa_com_lock_externo_retido() {
    let Some(f) = ReceiverFixture::new("timeout").await else {
        return;
    };
    f.reserve().await;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .after_connect(|db, _| {
            Box::pin(async move {
                sqlx::raw_sql("SET lock_timeout='50ms'; SET statement_timeout='250ms'")
                    .execute(db)
                    .await?;
                Ok(())
            })
        })
        .connect_with((*f.pool.connect_options()).clone())
        .await
        .unwrap();
    let portable = Arc::new(PortableJoin::new(
        f.portable_config.clone(),
        pool.clone(),
        f.receiver.clone(),
    ));
    let service =
        ReplicaService::preflight(f.config.clone(), portable, f.bundle.clone(), &f.receiver)
            .await
            .unwrap()
            .with_clock(Arc::new(|| 110));
    let mut external = f.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE federation_replica_resume_block IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *external)
        .await
        .unwrap();
    let key = f.origin.public_key_hex();
    let peer = SessionPeer {
        node_id: f.origin.node_id(),
        public_key: &key,
        protocol: 1,
    };

    for _ in 0..4 {
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            service.suspend_history(peer, 0, deadline()),
        )
        .await
        .expect("o limite preexistente governa o SQL");
        assert!(matches!(result, Err(EffectError::Timeout)), "{result:?}");
        let state: (String, String, i64) = sqlx::query_as(
            "SELECT current_setting('lock_timeout'), current_setting('statement_timeout'),
                (SELECT count(*) FROM pg_locks WHERE pid=pg_backend_pid() AND locktype='advisory')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(state, ("50ms".to_owned(), "250ms".to_owned(), 0));
    }
    external.commit().await.unwrap();
    assert_eq!(
        service.checkpoint(peer, deadline()).await.unwrap(),
        ReceiverState::Ready { after_seq: 0 }
    );
    assert_eq!(
        service.suspend_history(peer, 0, deadline()).await.unwrap(),
        ReceiverState::SuspendedHistory {
            blocked_next_seq: 1
        }
    );
}

#[tokio::test]
async fn prazo_durante_commit_tem_resultado_incerto_e_cursor_e_relido() {
    let Some(f) = ReceiverFixture::new("commit_deadline").await else {
        return;
    };
    f.reserve().await;
    let service = f.boot().await.unwrap();
    sqlx::raw_sql(
        "CREATE FUNCTION test_wait_at_commit() RETURNS trigger LANGUAGE plpgsql AS
           'BEGIN PERFORM pg_advisory_xact_lock(123456789); RETURN NEW; END';
         CREATE CONSTRAINT TRIGGER test_wait_at_commit
           AFTER INSERT ON federation_replica_resume_block
           DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION test_wait_at_commit();",
    )
    .execute(&f.pool)
    .await
    .unwrap();
    let mut gate = f.pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(123456789)")
        .execute(&mut *gate)
        .await
        .unwrap();
    let origin = f.origin.clone();
    let pending = tokio::spawn(async move {
        let key = origin.public_key_hex();
        let peer = SessionPeer {
            node_id: origin.node_id(),
            public_key: &key,
            protocol: 1,
        };
        let result = service
            .suspend_history(peer, 0, Instant::now() + Duration::from_millis(400))
            .await;
        (result, service)
    });
    tokio::time::timeout(Duration::from_millis(350), async {
        loop {
            let commit_waiting: bool = sqlx::query_scalar("SELECT EXISTS (
                SELECT FROM pg_stat_activity WHERE query='COMMIT' AND $1 = ANY(pg_blocking_pids(pid))
            )").bind(blocker).fetch_one(&f.pool).await.unwrap();
            if commit_waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("o COMMIT deve chegar à barreira");
    let (result, service) = pending.await.unwrap();
    assert!(
        matches!(result, Err(EffectError::CommitUnknown(_))),
        "{result:?}"
    );
    let key = f.origin.public_key_hex();
    let peer = SessionPeer {
        node_id: f.origin.node_id(),
        public_key: &key,
        protocol: 1,
    };
    assert_eq!(
        service.checkpoint(peer, deadline()).await.unwrap(),
        ReceiverState::Ready { after_seq: 0 }
    );
    gate.commit().await.unwrap();
    assert_eq!(
        service.suspend_history(peer, 0, deadline()).await.unwrap(),
        ReceiverState::SuspendedHistory {
            blocked_next_seq: 1
        }
    );
}

#[test]
fn origem_exige_dois_destinos_distintos_e_proibe_intervalo_de_pull() {
    let fixture = env("origin");
    assert!(ReplicaConfig::from_env_with(fixture.clone()).is_err());
    let mut fixture: Vec<_> = fixture
        .into_iter()
        .filter(|(key, _)| !key.ends_with("_PULL_INTERVAL_MS"))
        .collect();
    let destination_pins = "GODMESH_FEDERATION_M4_DESTINATION_PINS".to_owned();
    let pin = || {
        let identity = NodeIdentity::generate_in_memory().unwrap();
        format!("{}:{}", identity.node_id(), identity.public_key_hex())
    };
    let first = pin();
    fixture.push((destination_pins, format!("{first},{first}")));
    assert!(ReplicaConfig::from_env_with(fixture.clone()).is_err());
    fixture.last_mut().unwrap().1 = format!("{first},{}", pin());
    let parsed = ReplicaConfig::from_env_with(fixture).unwrap().unwrap();
    assert_eq!(parsed.role, Role::Origin);
    assert_eq!(parsed.destination_pins.len(), 2);
    assert!(parsed.pull_interval.is_none());
}

#[tokio::test]
async fn origem_prepara_pagina_sob_piso_e_acl_com_bytes_originais() {
    use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
    use godmesh_server::{
        auth::{Scope, Subject, VerifiedGrant},
        replication::{OriginAdmission, Series},
    };

    let Some(f) = ReceiverFixture::new("export").await else {
        return;
    };
    let source = Arc::new(PortableJoin::new(
        f.portable_config.clone(),
        f.pool.clone(),
        f.origin.clone(),
    ));
    source
        .import_genesis(&f.portable.local_manifest().unwrap(), JoinWindow::default())
        .await
        .unwrap();
    let other = NodeIdentity::generate_in_memory().unwrap();
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
    sqlx::query(
        "INSERT INTO federation_peer_scope (node_id,tenant_id,stream_id) VALUES ($1,$2,$3)",
    )
    .bind(f.origin.node_id())
    .bind(&ns.tenant_id)
    .bind(&ns.stream_id)
    .execute(&f.pool)
    .await
    .unwrap();
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
    let actual = [b" \t".to_vec(), f.frame(1), b"\n ".to_vec()].concat();
    let limits = Limits::new(8, 2048, 4096).unwrap();
    Replication::admit_origin_in_tx(
        &mut guard,
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
        &actual,
        || 100,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let service = ReplicaService::preflight(config, source, f.bundle.clone(), &f.origin)
        .await
        .unwrap()
        .with_clock(Arc::new(|| 110));
    let key = f.receiver.public_key_hex();
    let peer = SessionPeer {
        node_id: f.receiver.node_id(),
        public_key: &key,
        protocol: 1,
    };
    let batch = service
        .prepare_copy(peer, 0, limits, deadline(), Ok)
        .await
        .unwrap();
    assert_eq!(batch.frames, [actual]);
    assert!(!batch.has_more);
    sqlx::query("UPDATE federation_replica_acl SET revoked=true WHERE direction='export'")
        .execute(&f.pool)
        .await
        .unwrap();
    let denied = service
        .prepare_copy(
            peer,
            0,
            limits,
            deadline(),
            |_| -> Result<(), EffectError> {
                panic!("ACL revogada deve impedir preparação do frame")
            },
        )
        .await;
    assert!(matches!(
        denied,
        Err(EffectError::Replication(ReplicaError::AclDenied))
    ));
}
