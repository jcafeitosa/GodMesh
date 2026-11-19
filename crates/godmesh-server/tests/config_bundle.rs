mod common;

async fn database(suffix: &str) -> Option<sqlx::PgPool> {
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
    assert!(suffix.bytes().all(|b| b.is_ascii_alphabetic() || b == b'_'));
    let name = format!("m3b1_{}_{suffix}", std::process::id());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    Some(
        PgPoolOptions::new()
            .connect_with(base.database(&name))
            .await
            .unwrap(),
    )
}

#[tokio::test]
async fn migracao_incremental_preserva_historico_m1c() {
    let Some(pool) = database("upgrade").await else {
        return;
    };
    const M3_MIGRATION: &str = "0013_federation_config_bundle.sql";
    let dir = common::migrations_dir();
    let old = std::env::temp_dir().join(format!("godmesh-m3b1-{}", std::process::id()));
    std::fs::create_dir(&old).unwrap();
    for file in std::fs::read_dir(&dir).unwrap() {
        let file = file.unwrap();
        if file.path().extension().is_some_and(|ext| ext == "sql")
            && file.file_name().to_string_lossy().as_ref() < M3_MIGRATION
        {
            std::fs::copy(file.path(), old.join(file.file_name())).unwrap();
        }
    }
    let report = godmesh_server::migrate::migrate(&pool, &old).await.unwrap();
    assert_eq!(report.applied.len(), 12);
    let history = "SELECT nome, checksum, aplicado_em::text FROM schema_migration ORDER BY nome";
    let before: Vec<(String, String, String)> =
        sqlx::query_as(history).fetch_all(&pool).await.unwrap();
    std::fs::copy(dir.join(M3_MIGRATION), old.join(M3_MIGRATION)).unwrap();
    let report = godmesh_server::migrate::migrate(&pool, &old).await.unwrap();
    assert_eq!(report.applied, ["0013_federation_config_bundle.sql"]);
    let after: Vec<(String, String, String)> =
        sqlx::query_as(history).fetch_all(&pool).await.unwrap();
    assert_eq!(before, after[..12]);
    let repeat = godmesh_server::migrate::migrate(&pool, &old).await.unwrap();
    assert!(repeat.applied.is_empty());
    assert_eq!(repeat.skipped_count(), 13);
    std::fs::remove_dir_all(old).unwrap();
}

use std::{num::NonZeroUsize, sync::Arc};

use godmesh_federation::{
    NodeIdentity,
    config_bundle::{
        BundleBody, BundleKey, BundlePolicy, PinnedVoter, SoftwareVersion, vote_bytes,
    },
};
use godmesh_server::config_bundle::{
    Activation, Block, BundleService, Fence, Preflight, Published, Rejection, Role, ServicePolicy,
    WriteError, WriteOutcome,
};
use godmesh_server::portable_join::{PortableJoin, PortableJoinConfig};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};

const CLUSTER: &str = "cluster-teste";

struct Fixture {
    pool: PgPool,
    portable: PortableJoin,
    root: Arc<NodeIdentity>,
    config: PortableJoinConfig,
    voters: Vec<NodeIdentity>,
}

impl Fixture {
    async fn new(suffix: &str) -> Option<Self> {
        let pool = database(suffix).await?;
        godmesh_server::migrate::migrate(&pool, &common::migrations_dir())
            .await
            .unwrap();
        let root = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let pin = format!("{}:{}", root.node_id(), root.public_key_hex());
        let cfg = PortableJoinConfig::from_env_with(|k| {
            Some(
                match k {
                    "GODMESH_FEDERATION_GOVERNANCE" => "single_approver:cluster-teste",
                    "GODMESH_FEDERATION_GENESIS" => &pin,
                    "GODMESH_FEDERATION_REGION" => "lab",
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
        let portable = PortableJoin::new(cfg.clone(), pool.clone(), root.clone());
        let voters = (0..3)
            .map(|_| NodeIdentity::generate_in_memory().unwrap())
            .collect();
        Some(Self {
            pool,
            portable,
            root,
            config: cfg,
            voters,
        })
    }

    fn policy(&self, role: Role, software: SoftwareVersion) -> ServicePolicy {
        ServicePolicy {
            profile: 1,
            role,
            software,
            bundle: BundlePolicy {
                cluster: CLUSTER.into(),
                epoch: 7,
                max_frame_bytes: NonZeroUsize::new(8192).unwrap(),
                voters: self
                    .voters
                    .iter()
                    .map(|id| PinnedVoter {
                        node_id: id.node_id().into(),
                        public_key: id.public_key_hex(),
                    })
                    .collect(),
            },
        }
    }

    async fn enabled(&self, policy: ServicePolicy) -> BundleService {
        match BundleService::preflight(&self.pool, Some(&self.portable), Some(policy))
            .await
            .unwrap()
        {
            Preflight::Enabled(service) => service,
            Preflight::Disabled => panic!("serviço desabilitado"),
        }
    }

    async fn tx(&self) -> Transaction<'static, Postgres> {
        let mut tx = self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
            .await
            .unwrap();
        self.portable.lock_cluster(&mut tx).await.unwrap();
        tx
    }

    fn signed(
        &self,
        version: u64,
        floor: u64,
        due: i64,
        expires: i64,
        requires: SoftwareVersion,
    ) -> Vec<u8> {
        let body = BundleBody {
            key: BundleKey {
                cluster: CLUSTER.into(),
                namespace: "federation".into(),
                scope_id: String::new(),
                epoch: 7,
            },
            version,
            applies_at: due,
            expires_at: expires,
            required_signatures: 3,
            value: floor,
            effective_after: due,
            min_version: requires,
        };
        let digest = body.digest();
        let [major, minor, patch] = body.min_version.components();
        let signatures: Vec<Value> = self
            .voters
            .iter()
            .map(|id| {
                json!({
                    "signer_node_id": id.node_id(), "signature": id.sign_hex(&vote_bytes(&digest)),
                })
            })
            .collect();
        serde_json::to_vec(&json!({
            "profile": 1, "cluster": CLUSTER, "namespace": "federation", "scope_id": "", "epoch": body.key.epoch,
            "version": body.version, "applies_at": body.applies_at, "expires_at": body.expires_at, "required_signatures": 3,
            "entries": [{"key": "federation.minimum_protocol_version", "value": body.value,
                "effective_after": body.effective_after, "min_version": format!("{major}.{minor}.{patch}")}],
            "hash": hex::encode(digest), "signatures": signatures,
        })).unwrap()
    }

    async fn publish(
        &self,
        service: &BundleService,
        frame: &[u8],
        now: i64,
    ) -> Result<WriteOutcome, WriteError> {
        let mut tx = self.tx().await;
        let result = service
            .publish_in_tx(&mut tx, &self.portable, frame, now)
            .await;
        if result.is_ok() {
            tx.commit().await.unwrap();
        } else {
            tx.rollback().await.unwrap();
        }
        result
    }

    async fn fence(&self, service: &BundleService, protocol: u64, now: i64) -> Fence {
        let mut tx = self.tx().await;
        let result = service
            .classify_fence_in_tx(&mut tx, protocol, now)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        result
    }

    async fn snapshot(&self) -> Value {
        let row: String = sqlx::query_scalar(
            "SELECT jsonb_build_object(
              'state', (SELECT to_jsonb(s) FROM federation_config_state s),
              'frames', (SELECT jsonb_agg(to_jsonb(f) ORDER BY hash) FROM federation_config_frame f),
              'audit', (SELECT jsonb_agg(to_jsonb(a) ORDER BY id) FROM federation_config_audit a))::text",
        ).fetch_one(&self.pool).await.unwrap();
        serde_json::from_str(&row).unwrap()
    }
}

fn software(major: u64) -> SoftwareVersion {
    SoftwareVersion::new(major, 0, 0).unwrap()
}

#[tokio::test]
async fn preflight_sem_m1c_e_sem_estado_e_desabilitado_e_rejeita_estado_ou_config() {
    let Some(fx) = Fixture::new("preflight").await else {
        return;
    };
    let policy = fx.policy(Role::Publisher, software(1));
    assert!(matches!(
        BundleService::preflight(&fx.pool, None, None)
            .await
            .unwrap(),
        Preflight::Disabled
    ));
    assert!(
        BundleService::preflight(&fx.pool, None, Some(policy.clone()))
            .await
            .is_err()
    );
    let mut invalid = policy.clone();
    invalid.bundle.voters.pop();
    assert!(
        BundleService::preflight(&fx.pool, Some(&fx.portable), Some(invalid))
            .await
            .is_err()
    );
    let _service = fx.enabled(policy.clone()).await;
    assert!(
        BundleService::preflight(&fx.pool, None, None)
            .await
            .is_err()
    );
    assert!(
        BundleService::preflight(&fx.pool, Some(&fx.portable), None)
            .await
            .is_err()
    );
    let mut other = policy.clone();
    other.bundle.epoch += 1;
    assert!(
        BundleService::preflight(&fx.pool, Some(&fx.portable), Some(other))
            .await
            .is_err()
    );
    let mut reordered = policy;
    reordered.bundle.voters.reverse();
    let _restart = fx.enabled(reordered).await;
}

#[tokio::test]
async fn p_publica_vetor_m3a_recebe_ativa_e_le_sem_reescrita() {
    let Some(a) = Fixture::new("publisher").await else {
        return;
    };
    let b = Fixture::new("receiver").await.unwrap();
    let artifact: Value = serde_json::from_str(include_str!(
        "../../godmesh-federation/tests/fixtures/m3-a-vetor.json"
    ))
    .unwrap();
    let fixture = &artifact["quorum_3"];
    let pins: Vec<PinnedVoter> = fixture["voters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| PinnedVoter {
            node_id: v["node_id"].as_str().unwrap().into(),
            public_key: v["public_key"].as_str().unwrap().into(),
        })
        .collect();
    let mut pub_policy = a.policy(Role::Publisher, software(2));
    pub_policy.bundle.voters = pins;
    let mut sub_policy = pub_policy.clone();
    sub_policy.role = Role::Receiver;
    let publisher = a.enabled(pub_policy).await;
    let receiver = b.enabled(sub_policy).await;
    let frame = serde_json::to_vec_pretty(&fixture["bundle"]).unwrap();
    assert_eq!(
        a.publish(&publisher, &frame, 100).await.unwrap(),
        WriteOutcome::Accepted
    );
    assert_eq!(
        b.fence(&receiver, 2, 100).await,
        Fence::Blocked(Block::NoEffective)
    );
    let mut tx = a.tx().await;
    let Published::Frame {
        frame: served,
        version,
        hash,
    } = publisher
        .select_published_in_tx(&mut tx, None, 100)
        .await
        .unwrap()
    else {
        panic!("publicação indisponível")
    };
    assert_eq!(served, frame);
    assert_eq!(version, 9);
    assert_eq!(
        publisher
            .select_published_in_tx(&mut tx, Some(&hash), 100)
            .await
            .unwrap(),
        Published::NotModified { version, hash }
    );
    tx.commit().await.unwrap();
    assert_eq!(
        receiver.accept(&b.portable, &served, || 100).await.unwrap(),
        WriteOutcome::Accepted
    );
    assert_eq!(
        receiver.activate_due(&b.portable, || 119).await.unwrap(),
        Activation::NoChange
    );
    assert_eq!(
        b.fence(&receiver, 2, 120).await,
        Fence::Blocked(Block::PendingDue)
    );
    assert_eq!(
        receiver.activate_due(&b.portable, || 120).await.unwrap(),
        Activation::Activated
    );
    assert_eq!(
        b.fence(&receiver, 2, 120).await,
        Fence::Allowed {
            version,
            hash,
            floor: 2,
            expires_at: 200
        }
    );
    let before = b.snapshot().await;
    assert_eq!(
        receiver.accept(&b.portable, &served, || 200).await.unwrap(),
        WriteOutcome::Replay
    );
    assert_eq!(
        receiver.activate_due(&b.portable, || 200).await.unwrap(),
        Activation::NoChange
    );
    assert_eq!(
        b.fence(&receiver, 2, 200).await,
        Fence::Blocked(Block::Expired)
    );
    assert_eq!(before, b.snapshot().await);
    let mut tx = a.tx().await;
    assert_eq!(
        publisher
            .select_published_in_tx(&mut tx, Some(&hash), 200)
            .await
            .unwrap(),
        Published::Unavailable
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn b_evidencia_exigivel_e_substituicao_futura_fecham_o_fence() {
    let Some(fx) = Fixture::new("due").await else {
        return;
    };
    let service = fx.enabled(fx.policy(Role::Publisher, software(1))).await;
    let v1 = fx.signed(1, 1, 100, 210, software(1));
    let v2 = fx.signed(2, 2, 110, 210, software(1));
    let v3 = fx.signed(3, 3, 120, 135, software(2));
    assert_eq!(
        fx.publish(&service, &v1, 100).await.unwrap(),
        WriteOutcome::Accepted
    );
    assert_eq!(
        service.activate_due(&fx.portable, || 100).await.unwrap(),
        Activation::Activated
    );
    assert_eq!(
        fx.publish(&service, &v2, 101).await.unwrap(),
        WriteOutcome::Accepted
    );
    assert_eq!(
        fx.publish(&service, &v3, 102).await.unwrap(),
        WriteOutcome::IncompatibleRecorded
    );
    assert_eq!(
        service.activate_due(&fx.portable, || 119).await.unwrap(),
        Activation::Activated
    );
    assert!(matches!(
        fx.fence(&service, 2, 119).await,
        Fence::Allowed { version: 2, .. }
    ));
    assert_eq!(
        fx.fence(&service, 2, 120).await,
        Fence::Blocked(Block::Incompatible)
    );
    assert_eq!(
        fx.fence(&service, 2, 135).await,
        Fence::Blocked(Block::Incompatible)
    );
    let v4 = fx.signed(4, 4, 150, 210, software(1));
    assert_eq!(
        fx.publish(&service, &v4, 140).await.unwrap(),
        WriteOutcome::Accepted
    );
    assert_eq!(
        fx.fence(&service, 2, 140).await,
        Fence::Blocked(Block::Recovering)
    );
    assert_eq!(
        service.activate_due(&fx.portable, || 150).await.unwrap(),
        Activation::Activated
    );
    assert_eq!(
        fx.fence(&service, 2, 150).await,
        Fence::Blocked(Block::BelowFloor)
    );
    assert!(matches!(
        fx.fence(&service, 4, 150).await,
        Fence::Allowed { floor: 4, .. }
    ));
}

fn rejected(result: Result<WriteOutcome, WriteError>, expected: Rejection) {
    assert!(
        matches!(result, Err(WriteError::Rejected(reason)) if reason == expected),
        "{result:?}"
    );
}

#[tokio::test]
async fn n_papel_restringe_replay_conflito_e_incompatibilidade() {
    for (role, suffix) in [(Role::Publisher, "role_a"), (Role::Receiver, "role_b")] {
        let Some(fx) = Fixture::new(suffix).await else {
            return;
        };
        let service = fx.enabled(fx.policy(role, software(1))).await;
        let original = fx.signed(1, 1, 100, 200, software(1));
        let alternative = fx.signed(1, 2, 100, 200, software(1));
        let incompatible = fx.signed(3, 3, 130, 210, software(2));
        match role {
            Role::Publisher => assert_eq!(
                fx.publish(&service, &original, 100).await.unwrap(),
                WriteOutcome::Accepted
            ),
            Role::Receiver => assert_eq!(
                service
                    .accept(&fx.portable, &original, || 100)
                    .await
                    .unwrap(),
                WriteOutcome::Accepted
            ),
        }
        let before = fx.snapshot().await;
        for frame in [&original, &alternative, &incompatible] {
            let mut tx = fx.tx().await;
            let result = match role {
                Role::Publisher => {
                    service
                        .accept_in_tx(&mut tx, &fx.portable, frame, 110)
                        .await
                }
                Role::Receiver => {
                    service
                        .publish_in_tx(&mut tx, &fx.portable, frame, 110)
                        .await
                }
            };
            rejected(result, Rejection::WrongRole);
            let wrong = if role == Role::Publisher {
                Role::Receiver
            } else {
                Role::Publisher
            };
            rejected(
                service
                    .resolve_conflict_in_tx(&mut tx, &fx.portable, frame, wrong, "auditor", 110)
                    .await,
                Rejection::WrongRole,
            );
            tx.commit().await.unwrap();
        }
        assert_eq!(before, fx.snapshot().await);
    }
}

#[tokio::test]
async fn n_conflito_historico_rollback_atomico_e_resolucao_auditada() {
    use godmesh_federation::config_bundle::BundleError;
    let Some(fx) = Fixture::new("conflict").await else {
        return;
    };
    let policy = fx.policy(Role::Publisher, software(1));
    let service = fx.enabled(policy.clone()).await;
    let v1 = fx.signed(1, 1, 100, 150, software(1));
    let conflict = fx.signed(1, 2, 100, 150, software(1));
    let v3 = fx.signed(3, 3, 180, 240, software(1));
    let v2 = fx.signed(2, 2, 160, 240, software(1));
    assert_eq!(
        fx.publish(&service, &v1, 100).await.unwrap(),
        WriteOutcome::Accepted
    );
    assert_eq!(
        service.activate_due(&fx.portable, || 100).await.unwrap(),
        Activation::Activated
    );
    assert_eq!(
        fx.publish(&service, &v3, 120).await.unwrap(),
        WriteOutcome::Accepted
    );
    let before = fx.snapshot().await;
    rejected(fx.publish(&service, &v2, 120).await, Rejection::Rollback);
    let mut invalid: Value = serde_json::from_slice(&v3).unwrap();
    invalid["signatures"].as_array_mut().unwrap().pop();
    rejected(
        fx.publish(&service, &serde_json::to_vec(&invalid).unwrap(), 120)
            .await,
        Rejection::Codec(BundleError::InsufficientQuorum),
    );
    let valid: Value = serde_json::from_slice(&v3).unwrap();
    let mut invalid = valid.clone();
    invalid["signatures"][0]["signature"] = json!("0".repeat(128));
    rejected(
        fx.publish(&service, &serde_json::to_vec(&invalid).unwrap(), 120)
            .await,
        Rejection::Codec(BundleError::InvalidSignature),
    );
    let mut invalid = valid.clone();
    invalid["epoch"] = json!(6);
    rejected(
        fx.publish(&service, &serde_json::to_vec(&invalid).unwrap(), 120)
            .await,
        Rejection::Codec(BundleError::EpochMismatch),
    );
    let mut invalid = valid;
    invalid["scope_id"] = json!("outro");
    rejected(
        fx.publish(&service, &serde_json::to_vec(&invalid).unwrap(), 120)
            .await,
        Rejection::Codec(BundleError::InvalidField("scope_id")),
    );
    assert_eq!(before, fx.snapshot().await);

    let mut tx = fx.tx().await;
    assert_eq!(
        service
            .publish_in_tx(&mut tx, &fx.portable, &conflict, 151)
            .await
            .unwrap(),
        WriteOutcome::ConflictRecorded
    );
    tx.rollback().await.unwrap();
    assert_eq!(before, fx.snapshot().await);
    sqlx::query("ALTER TABLE federation_config_audit ADD CONSTRAINT fail_conflict_audit CHECK (transition <> 'conflict')")
        .execute(&fx.pool).await.unwrap();
    assert!(matches!(
        fx.publish(&service, &conflict, 151).await,
        Err(WriteError::Transient(_))
    ));
    assert_eq!(before, fx.snapshot().await);
    sqlx::query("ALTER TABLE federation_config_audit DROP CONSTRAINT fail_conflict_audit")
        .execute(&fx.pool)
        .await
        .unwrap();

    assert_eq!(
        fx.publish(&service, &conflict, 151).await.unwrap(),
        WriteOutcome::ConflictRecorded
    );
    let blocked = fx.snapshot().await;
    assert_eq!(
        blocked["audit"].as_array().unwrap().last().unwrap()["transition"],
        "conflict"
    );
    assert_eq!(
        fx.publish(&service, &conflict, 151).await.unwrap(),
        WriteOutcome::ConflictKnown
    );
    assert_eq!(
        fx.publish(&service, &v1, 151).await.unwrap(),
        WriteOutcome::Replay
    );
    assert_eq!(blocked, fx.snapshot().await);
    let service = fx.enabled(policy.clone()).await;
    assert_eq!(
        fx.fence(&service, 5, 250).await,
        Fence::Blocked(Block::Conflict)
    );
    let mut tx = fx.tx().await;
    assert_eq!(
        service
            .select_published_in_tx(&mut tx, None, 250)
            .await
            .unwrap(),
        Published::BlockedConflict
    );
    rejected(
        service
            .resolve_conflict_in_tx(&mut tx, &fx.portable, &v3, Role::Publisher, "auditor", 156)
            .await,
        Rejection::Rollback,
    );
    tx.rollback().await.unwrap();
    let v5 = fx.signed(5, 5, 160, 240, software(1));
    let mut tx = fx.tx().await;
    assert_eq!(
        service
            .resolve_conflict_in_tx(&mut tx, &fx.portable, &v5, Role::Publisher, "auditor", 156)
            .await
            .unwrap(),
        WriteOutcome::Accepted
    );
    tx.commit().await.unwrap();
    let resolution = fx.snapshot().await;
    let audit = resolution["audit"].as_array().unwrap().last().unwrap();
    assert_eq!(audit["actor"], "auditor");
    assert_eq!(audit["happened_at"], 156);
    assert_eq!(
        fx.fence(&service, 5, 159).await,
        Fence::Blocked(Block::Recovering)
    );
    let mut tx = fx.tx().await;
    rejected(
        service
            .resolve_conflict_in_tx(&mut tx, &fx.portable, &v5, Role::Receiver, "auditor", 160)
            .await,
        Rejection::WrongRole,
    );
    assert_eq!(
        service
            .resolve_conflict_in_tx(&mut tx, &fx.portable, &v5, Role::Publisher, "auditor", 250)
            .await
            .unwrap(),
        WriteOutcome::Replay
    );
    tx.commit().await.unwrap();
    assert_eq!(resolution, fx.snapshot().await);
    assert_eq!(
        service.activate_due(&fx.portable, || 160).await.unwrap(),
        Activation::Activated
    );
    assert!(matches!(
        fx.fence(&service, 5, 160).await,
        Fence::Allowed { floor: 5, .. }
    ));
    let mut smaller = policy.clone();
    smaller.bundle.max_frame_bytes = NonZeroUsize::new(1).unwrap();
    assert!(
        BundleService::preflight(&fx.pool, Some(&fx.portable), Some(smaller))
            .await
            .is_err()
    );

    // A referência efetiva deve corresponder à última ativação auditada.
    // Uma indisponibilidade ou inconsistência impede a retomada no preflight.
    sqlx::query("UPDATE federation_config_state SET effective = (SELECT hash FROM federation_config_frame WHERE version = 1 AND accepted), pending = accepted")
        .execute(&fx.pool).await.unwrap();
    assert!(
        BundleService::preflight(&fx.pool, Some(&fx.portable), Some(policy))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn b_reconsidera_mesmo_hash_com_software_novo_antes_e_depois_da_exigencia() {
    for (now, suffix) in [(119, "before_due"), (120, "at_due")] {
        let Some(fx) = Fixture::new(suffix).await else {
            return;
        };
        let old = fx.enabled(fx.policy(Role::Receiver, software(1))).await;
        let first = fx.signed(1, 1, 100, 200, software(1));
        let future = fx.signed(3, 3, 120, 190, software(2));
        assert_eq!(
            old.accept(&fx.portable, &first, || 100).await.unwrap(),
            WriteOutcome::Accepted
        );
        assert_eq!(
            old.activate_due(&fx.portable, || 100).await.unwrap(),
            Activation::Activated
        );
        let before = fx.snapshot().await;
        let mut tx = fx.tx().await;
        assert_eq!(
            old.accept_in_tx(&mut tx, &fx.portable, &future, 101)
                .await
                .unwrap(),
            WriteOutcome::IncompatibleRecorded
        );
        tx.rollback().await.unwrap();
        assert_eq!(before, fx.snapshot().await);
        assert_eq!(
            old.accept(&fx.portable, &future, || 102).await.unwrap(),
            WriteOutcome::IncompatibleRecorded
        );
        let recorded = fx.snapshot().await;
        assert_eq!(
            old.accept(&fx.portable, &future, || 103).await.unwrap(),
            WriteOutcome::IncompatibleKnown
        );
        assert_eq!(recorded, fx.snapshot().await);

        let upgraded = fx.enabled(fx.policy(Role::Receiver, software(2))).await;
        // O mesmo hash pode chegar com espaçamento diferente. A evidência
        // conserva o frame que o receptor autenticou na primeira observação.
        let pretty: Value = serde_json::from_slice(&future).unwrap();
        let pretty = serde_json::to_vec_pretty(&pretty).unwrap();
        assert_eq!(
            upgraded
                .accept(&fx.portable, &pretty, || now)
                .await
                .unwrap(),
            WriteOutcome::Accepted
        );
        let frames = fx.snapshot().await;
        let stored = frames["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["version"] == 3)
            .unwrap();
        assert_eq!(
            stored["frame"].as_str().unwrap(),
            format!("\\x{}", hex::encode(&future))
        );
        if now < 120 {
            assert!(matches!(
                fx.fence(&upgraded, 1, now).await,
                Fence::Allowed { version: 1, .. }
            ));
        } else {
            assert_eq!(
                fx.fence(&upgraded, 1, now).await,
                Fence::Blocked(Block::Recovering)
            );
        }
        assert_eq!(
            upgraded.activate_due(&fx.portable, || 120).await.unwrap(),
            Activation::Activated
        );
        assert_eq!(
            fx.fence(&upgraded, 1, 120).await,
            Fence::Blocked(Block::BelowFloor)
        );
        let expired_unseen = fx.signed(500, 100, 100, 121, software(2));
        let before = fx.snapshot().await;
        rejected(
            upgraded.accept(&fx.portable, &expired_unseen, || 121).await,
            Rejection::Expired,
        );
        assert_eq!(before, fx.snapshot().await);
    }
}

async fn wait_for_m1c_lock_waiter(pool: &PgPool) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND NOT granted
                    AND database = (SELECT oid FROM pg_database WHERE datname = current_database()))",
            ).fetch_one(pool).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("contendor deve aguardar o lock M1C");
}

#[tokio::test]
async fn f_classificacao_e_ativacao_observam_a_ordem_do_commit_e_o_relogio_do_lock() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let Some(fx) = Fixture::new("fence").await else {
        return;
    };
    let fx = Arc::new(fx);
    let svc = Arc::new(fx.enabled(fx.policy(Role::Publisher, software(1))).await);
    let v1 = fx.signed(1, 1, 100, 200, software(1));
    let v2 = fx.signed(2, 2, 120, 200, software(1));
    fx.publish(&svc, &v1, 100).await.unwrap();
    assert_eq!(
        svc.activate_due(&fx.portable, || 100).await.unwrap(),
        Activation::Activated
    );
    fx.publish(&svc, &v2, 105).await.unwrap();
    sqlx::query("CREATE TABLE test_fenced_effect (protocol bigint PRIMARY KEY)")
        .execute(&fx.pool)
        .await
        .unwrap();

    let mut effect = fx.tx().await;
    assert!(matches!(
        svc.classify_fence_in_tx(&mut effect, 1, 119).await.unwrap(),
        Fence::Allowed { floor: 1, .. }
    ));
    sqlx::query("INSERT INTO test_fenced_effect VALUES (1)")
        .execute(&mut *effect)
        .await
        .unwrap();

    let calls = Arc::new(AtomicUsize::new(0));
    let activate = {
        let (fx, svc, calls) = (fx.clone(), svc.clone(), calls.clone());
        tokio::spawn(async move {
            svc.activate_due(&fx.portable, || {
                calls.fetch_add(1, Ordering::SeqCst);
                120
            })
            .await
            .unwrap()
        })
    };
    wait_for_m1c_lock_waiter(&fx.pool).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    effect.commit().await.unwrap();
    assert_eq!(activate.await.unwrap(), Activation::Activated);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        fx.fence(&svc, 1, 120).await,
        Fence::Blocked(Block::BelowFloor)
    );
    let protocol: i64 = sqlx::query_scalar("SELECT protocol FROM test_fenced_effect")
        .fetch_one(&fx.pool)
        .await
        .unwrap();
    assert_eq!(protocol, 1);
    let mut unlocked = fx.pool.begin().await.unwrap();
    assert!(matches!(
        svc.classify_fence_in_tx(&mut unlocked, 10, 120).await,
        Err(WriteError::Rejected(Rejection::LockRequired))
    ));
    unlocked.rollback().await.unwrap();
    let mut locked = fx.tx().await;
    let backend: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *locked)
        .await
        .unwrap();
    let killed: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1)")
        .bind(backend)
        .fetch_one(&fx.pool)
        .await
        .unwrap();
    assert!(killed);
    assert!(matches!(
        svc.classify_fence_in_tx(&mut locked, 10, 120).await,
        Err(WriteError::Transient(_))
    ));
}

struct Member {
    identity: Arc<NodeIdentity>,
    chain: String,
    lease: godmesh_federation::join_authority::JoinValidityV1,
    revocation: godmesh_federation::join_authority::JoinRevocationV1,
}

async fn import_member(fx: &Fixture) -> Member {
    use godmesh_federation::{join_authority::JoinRevocationV1, join_chain::JoinChainV2, now_unix};
    use godmesh_server::{
        federation::{JoinWindow, submit_join_request},
        portable_join::JoinSignatureRequest,
    };

    let identity = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let mut manifest = fx.portable.local_manifest().unwrap();
    manifest.sign(&identity);
    let window = JoinWindow::default();
    let now = now_unix();
    let id = submit_join_request(&fx.pool, window, &manifest, now)
        .await
        .unwrap()
        .request
        .join_request_id;
    let offer = fx
        .portable
        .sign_offer(
            JoinSignatureRequest {
                join_request_id: id.clone(),
                manifest,
                can_approve: false,
                not_before: now,
                expires_at: now + 900,
                epoch: 1,
            },
            window,
            16384,
        )
        .await
        .unwrap();
    let (_, chain) = fx
        .portable
        .approve(&id, &offer, "test", window, 16384)
        .await
        .unwrap();
    let lease = fx.portable.issue_validity(&chain).await.unwrap();
    fx.portable
        .import_chain(
            identity.node_id(),
            &identity.public_key_hex(),
            &chain,
            &lease,
        )
        .await
        .unwrap();
    let parsed: JoinChainV2 = serde_json::from_str(&chain).unwrap();
    let mut revocation = JoinRevocationV1 {
        cluster: CLUSTER.into(),
        root_node_id: fx.root.node_id().into(),
        target_digest: parsed.links[0].certificate.digest().unwrap(),
        revoker_node_id: String::new(),
        epoch: 1,
        signature: String::new(),
    };
    revocation.sign(&fx.root);
    Member {
        identity,
        chain,
        lease,
        revocation,
    }
}

#[derive(Clone, Copy)]
enum ProtectedOperation {
    Frame,
    Conditional,
    Publish,
    Resolve,
}

#[tokio::test]
async fn f_revogacao_serializa_frame_condicional_e_os_dois_puts() {
    use godmesh_federation::config_bundle::authenticate_bundle;
    use godmesh_server::portable_join::PortableJoinError;

    for (operation, suffix) in [
        (ProtectedOperation::Frame, "rev_frame"),
        (ProtectedOperation::Conditional, "rev_conditional"),
        (ProtectedOperation::Publish, "rev_publish"),
        (ProtectedOperation::Resolve, "rev_resolve"),
    ] {
        let Some(fx) = Fixture::new(suffix).await else {
            return;
        };
        let fx = Arc::new(fx);
        let member = Arc::new(import_member(&fx).await);
        let now = member.lease.issued_at;
        let policy = fx.policy(Role::Publisher, software(1));
        let v1 = fx.signed(1, 1, now, now + 300, software(1));
        let v2 = fx.signed(2, 2, now, now + 300, software(1));
        let hash = *authenticate_bundle(&v1, &policy.bundle).unwrap().hash();
        let service = fx.enabled(policy).await;
        assert_eq!(
            fx.publish(&service, &v1, now).await.unwrap(),
            WriteOutcome::Accepted
        );
        if matches!(operation, ProtectedOperation::Resolve) {
            let conflict = fx.signed(1, 3, now, now + 300, software(1));
            assert_eq!(
                fx.publish(&service, &conflict, now).await.unwrap(),
                WriteOutcome::ConflictRecorded
            );
        }

        let mut authorized = fx.tx().await;
        fx.portable
            .authorize_local_at(&mut authorized, now)
            .await
            .unwrap();
        assert_eq!(
            fx.portable
                .authorize_peer_at(
                    &mut authorized,
                    member.identity.node_id(),
                    &member.identity.public_key_hex(),
                    now
                )
                .await
                .unwrap(),
            Some(member.lease.expires_at)
        );
        let revoke = {
            let (fx, member) = (fx.clone(), member.clone());
            tokio::spawn(async move {
                fx.portable
                    .revoke(&member.chain, &member.revocation)
                    .await
                    .unwrap()
            })
        };
        wait_for_m1c_lock_waiter(&fx.pool).await;
        match operation {
            ProtectedOperation::Frame => assert_eq!(
                service
                    .select_published_in_tx(&mut authorized, None, now)
                    .await
                    .unwrap(),
                Published::Frame {
                    frame: v1.clone(),
                    version: 1,
                    hash
                }
            ),
            ProtectedOperation::Conditional => assert_eq!(
                service
                    .select_published_in_tx(&mut authorized, Some(&hash), now)
                    .await
                    .unwrap(),
                Published::NotModified { version: 1, hash }
            ),
            ProtectedOperation::Publish => assert_eq!(
                service
                    .publish_in_tx(&mut authorized, &fx.portable, &v2, now)
                    .await
                    .unwrap(),
                WriteOutcome::Accepted
            ),
            ProtectedOperation::Resolve => assert_eq!(
                service
                    .resolve_conflict_in_tx(
                        &mut authorized,
                        &fx.portable,
                        &v2,
                        Role::Publisher,
                        "test",
                        now
                    )
                    .await
                    .unwrap(),
                WriteOutcome::Accepted
            ),
        }
        assert!(!revoke.is_finished());
        authorized.commit().await.unwrap();
        assert!(revoke.await.unwrap().0);

        // A fronteira só chamaria leitura ou PUT após autorização na mesma
        // transação. Mesmo ETag ou replay conhecido não contorna a revogação.
        let before = fx.snapshot().await;
        let mut denied = fx.tx().await;
        assert!(matches!(
            fx.portable
                .authorize_peer_at(
                    &mut denied,
                    member.identity.node_id(),
                    &member.identity.public_key_hex(),
                    now
                )
                .await,
            Err(PortableJoinError::Revoked)
        ));
        denied.rollback().await.unwrap();
        assert_eq!(before, fx.snapshot().await);
    }
}

#[tokio::test]
async fn f_autoridade_propria_e_remota_usam_o_segundo_explicito_e_o_mesmo_lock() {
    use godmesh_server::portable_join::PortableJoinError;
    let Some(fx) = Fixture::new("explicit_root").await else {
        return;
    };
    let member = import_member(&fx).await;
    let member_pool = database("explicit_member").await.unwrap();
    godmesh_server::migrate::migrate(&member_pool, &common::migrations_dir())
        .await
        .unwrap();
    let local = PortableJoin::new(
        fx.config.clone(),
        member_pool.clone(),
        member.identity.clone(),
    );
    assert!(
        local
            .import_chain(
                member.identity.node_id(),
                &member.identity.public_key_hex(),
                &member.chain,
                &member.lease
            )
            .await
            .unwrap()
    );
    let mut local_tx = member_pool.begin().await.unwrap();
    assert!(matches!(
        local
            .authorize_local_at(&mut local_tx, member.lease.issued_at)
            .await,
        Err(PortableJoinError::Configuration(_))
    ));
    local.lock_cluster(&mut local_tx).await.unwrap();
    let expiry = member.lease.expires_at;
    assert_eq!(
        local
            .authorize_local_at(&mut local_tx, expiry - 1)
            .await
            .unwrap(),
        Some(expiry)
    );
    assert!(matches!(
        local.authorize_local_at(&mut local_tx, expiry).await,
        Err(PortableJoinError::Validity)
    ));
    local_tx.rollback().await.unwrap();
    let mut peer_tx = fx.tx().await;
    assert_eq!(
        fx.portable
            .authorize_peer_at(
                &mut peer_tx,
                member.identity.node_id(),
                &member.identity.public_key_hex(),
                expiry - 1
            )
            .await
            .unwrap(),
        Some(expiry)
    );
    assert!(matches!(
        fx.portable
            .authorize_peer_at(
                &mut peer_tx,
                member.identity.node_id(),
                &member.identity.public_key_hex(),
                expiry
            )
            .await,
        Err(PortableJoinError::Validity)
    ));
    peer_tx.rollback().await.unwrap();
}

#[tokio::test]
async fn f_selecao_observa_publicacao_e_conflito_apenas_apos_commit() {
    use godmesh_federation::config_bundle::authenticate_bundle;

    let Some(fx) = Fixture::new("selection").await else {
        return;
    };
    let fx = Arc::new(fx);
    let policy = fx.policy(Role::Publisher, software(1));
    let v1 = fx.signed(1, 1, 100, 180, software(1));
    let v2 = fx.signed(2, 2, 120, 200, software(1));
    let alternative = fx.signed(2, 3, 120, 200, software(1));
    let hash1 = *authenticate_bundle(&v1, &policy.bundle).unwrap().hash();
    let hash2 = *authenticate_bundle(&v2, &policy.bundle).unwrap().hash();
    let service = Arc::new(fx.enabled(policy).await);
    let mut first = fx.tx().await;
    assert_eq!(
        service
            .select_published_in_tx(&mut first, None, 100)
            .await
            .unwrap(),
        Published::Unavailable
    );
    first.commit().await.unwrap();
    assert_eq!(
        fx.publish(&service, &v1, 100).await.unwrap(),
        WriteOutcome::Accepted
    );
    let before_replay = fx.snapshot().await;
    assert_eq!(
        fx.publish(&service, &v1, 100).await.unwrap(),
        WriteOutcome::Replay
    );
    assert_eq!(before_replay, fx.snapshot().await);

    let mut reader = fx.tx().await;
    let writer = {
        let (fx, service, v2) = (fx.clone(), service.clone(), v2.clone());
        tokio::spawn(async move { fx.publish(&service, &v2, 110).await.unwrap() })
    };
    wait_for_m1c_lock_waiter(&fx.pool).await;
    let prepared = service
        .select_published_in_tx(&mut reader, None, 110)
        .await
        .unwrap();
    reader.commit().await.unwrap();
    assert_eq!(writer.await.unwrap(), WriteOutcome::Accepted);
    assert_eq!(
        prepared,
        Published::Frame {
            frame: v1,
            version: 1,
            hash: hash1
        }
    );

    let mut witness = fx.tx().await;
    assert_eq!(
        service
            .select_published_in_tx(&mut witness, Some(&hash1), 110)
            .await
            .unwrap(),
        Published::Frame {
            frame: v2,
            version: 2,
            hash: hash2
        }
    );
    assert_eq!(
        service
            .publish_in_tx(&mut witness, &fx.portable, &alternative, 110)
            .await
            .unwrap(),
        WriteOutcome::ConflictRecorded
    );
    let reader = {
        let (fx, service) = (fx.clone(), service.clone());
        tokio::spawn(async move {
            let mut tx = fx.tx().await;
            let prepared = service
                .select_published_in_tx(&mut tx, Some(&hash2), 110)
                .await
                .unwrap();
            tx.commit().await.unwrap();
            prepared
        })
    };
    wait_for_m1c_lock_waiter(&fx.pool).await;
    assert!(!reader.is_finished());
    witness.commit().await.unwrap();
    assert_eq!(reader.await.unwrap(), Published::BlockedConflict);
}

#[tokio::test]
async fn n_conflito_com_evidencia_incompativel_precede_ausencia_de_publicacao() {
    let Some(fx) = Fixture::new("evidence_conflict").await else {
        return;
    };
    let policy = fx.policy(Role::Publisher, software(1));
    let svc = fx.enabled(policy.clone()).await;
    let v1 = fx.signed(1, 1, 100, 140, software(2));
    let alternative = fx.signed(1, 2, 100, 140, software(1));
    assert_eq!(
        fx.publish(&svc, &v1, 100).await.unwrap(),
        WriteOutcome::IncompatibleRecorded
    );
    let mut tx = fx.tx().await;
    assert_eq!(
        svc.select_published_in_tx(&mut tx, None, 100)
            .await
            .unwrap(),
        Published::Unavailable
    );
    tx.commit().await.unwrap();
    assert_eq!(
        fx.publish(&svc, &alternative, 140).await.unwrap(),
        WriteOutcome::ConflictRecorded
    );
    let svc = fx.enabled(policy).await;
    let mut tx = fx.tx().await;
    assert_eq!(
        svc.select_published_in_tx(&mut tx, None, 140)
            .await
            .unwrap(),
        Published::BlockedConflict
    );
    tx.commit().await.unwrap();
    assert_eq!(
        fx.fence(&svc, 10, 140).await,
        Fence::Blocked(Block::Conflict)
    );
}
