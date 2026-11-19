//! Operações administrativas da fixture. Este entrypoint nunca abre listeners.

use std::{collections::BTreeMap, io::Write, time::Duration};

use godmesh_federation::{
    NodeIdentity,
    restore_evidence::{BackupKind, Fence, RestoreHighWaterV1, RestorePromotionV1},
};
use godmesh_server::{
    config_bundle::{BundleService, Preflight},
    config_operational::ConfigPlaneConfig,
    portable_join::{PortableJoin, PortableJoinConfig},
    replica_operational::{
        ReplicaConfig,
        restore_lab::{
            Binding, GenerationLogins, capture_gap, observe_cut, observe_opening_origin,
            observe_origin,
        },
        snapshot_harness::Window,
    },
};
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    key: RestoreKey,
    control: PgAllocation,
    limit_bytes: NonZeroUsize,
    timeout_ms: NonZeroU32,
    action: OperatorAction,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum OperatorAction {
    Seed {
        target_db_id: String,
        logins: LoginsInput,
    },
    Publish(Box<PublishInput>),
    SignPromotion {
        high_water: PathBuf,
        expected_reconciliation: String,
        signer: PathBuf,
        output: PathBuf,
        valid_seconds: NonZeroU32,
    },
    ObserveOrigin {
        phase: ObservationPhase,
        origin: Box<Actor>,
        destination_public_key: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ObservationPhase {
    Cas,
    Opening,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Actor {
    database: PgAllocation,
    config: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginsInput {
    target_login: String,
    observer_login: String,
    cas_login: String,
}

impl From<LoginsInput> for GenerationLogins {
    fn from(value: LoginsInput) -> Self {
        Self {
            target_login: value.target_login,
            observer_login: value.observer_login,
            cas_login: value.cas_login,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishInput {
    origin: Actor,
    cut: Actor,
    restored: PgAllocation,
    old_login: String,
    logins: LoginsInput,
    backup: PathBuf,
    backup_prefix: u64,
    backup_generation: u64,
    signer: PathBuf,
    high_water: PathBuf,
    gap: Option<SnapshotInput>,
    valid_seconds: NonZeroU32,
}

async fn connect(allocation: &PgAllocation, env: &str) -> Result<PgPool, String> {
    let url = std::env::var(env).map_err(|_| format!("fixture exige {env}"))?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(allocation.connect_options(&url)?)
        .await
        .map_err(|_| "banco administrativo da fixture indisponível".to_owned())?;
    // O operador audita as duas regras da fixture. O roteiro confirma
    // a regra carregada com tentativas reais sem credencial.
    let scram_only: bool = sqlx::query_scalar(
        "SELECT count(*) = 2
          AND count(*) FILTER (
             WHERE type = 'host' AND database = ARRAY['all'] AND user_name = ARRAY['all']
               AND address = '127.0.0.1' AND netmask = '255.255.255.255'
               AND auth_method = 'scram-sha-256' AND error IS NULL
          ) = 1
          AND count(*) FILTER (
             WHERE type = 'local' AND database = ARRAY['all'] AND user_name = ARRAY['all']
               AND auth_method = 'reject' AND error IS NULL
          ) = 1
         FROM pg_catalog.pg_hba_file_rules",
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| "regras de autenticação da fixture indisponíveis".to_owned())?;
    if !scram_only {
        return Err("a fixture exige SCRAM para todos os papéis TCP".to_owned());
    }
    Ok(pool)
}

async fn actor(input: &Actor, pool: PgPool) -> Result<(Arc<NodeIdentity>, ReplicaService), String> {
    let env = |name: &str| input.config.get(name).cloned();
    let key_path = input
        .config
        .get("GODMESH_NODE_KEY")
        .ok_or("identidade ausente na fixture")?;
    let identity = Arc::new(NodeIdentity::load(key_path).map_err(closed)?);
    let portable = PortableJoinConfig::from_env_with(env)
        .map_err(closed)?
        .ok_or("fixture de restore exige M1C")?;
    let bundle = ConfigPlaneConfig::from_env_with(env)
        .map_err(closed)?
        .ok_or("fixture de restore exige M3")?;
    let replica = ReplicaConfig::from_env_with(input.config.clone())
        .map_err(closed)?
        .ok_or("fixture de restore exige M4")?;
    PortableJoin::preflight(&pool, Some(&portable))
        .await
        .map_err(closed)?;
    let portable = Arc::new(PortableJoin::new(
        portable,
        pool.clone(),
        Arc::clone(&identity),
    ));
    let Preflight::Enabled(bundle) =
        BundleService::preflight(&pool, Some(&portable), Some(bundle.policy))
            .await
            .map_err(closed)?
    else {
        return Err("fixture de restore exige M3".to_owned());
    };
    let service = ReplicaService::preflight(replica, portable, Arc::new(bundle), &identity)
        .await
        .map_err(closed)?;
    Ok((identity, service))
}

fn peer(service: &ReplicaService) -> SessionPeer<'_> {
    let pin = service.local_pin();
    SessionPeer {
        node_id: &pin.node_id,
        public_key: &pin.public_key,
        protocol: 1,
    }
}

async fn check_fence(
    pool: &PgPool,
    old_login: &str,
    key: &RestoreKey,
    generation: u64,
    restored: &PgPool,
) -> Result<(bool, bool, bool, u64, u64), String> {
    let closed: bool = sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM public.federation_restore_local
             WHERE (fixture_id, cluster, local_node, origin_node, tenant_id, stream_id,
                    partition, authority_epoch, generation) = ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               AND phase='retired_closed'
         )",
    )
    .bind(&key.fixture_id)
    .bind(&key.cluster)
    .bind(&key.local_node)
    .bind(&key.origin_node)
    .bind(&key.tenant_id)
    .bind(&key.stream_id)
    .bind(i32::try_from(key.partition).map_err(super::closed)?)
    .bind(i64::try_from(key.authority_epoch).map_err(super::closed)?)
    .bind(i64::try_from(generation).map_err(super::closed)?)
    .fetch_one(pool)
    .await
    .map_err(super::closed)?;
    let (nologin, sessions, prepared): (bool, i64, i64) = sqlx::query_as(
        "SELECT NOT rolcanlogin, (SELECT count(*) FROM pg_stat_activity WHERE usename=$1),
                (SELECT count(*) FROM pg_prepared_xacts WHERE owner=$1)
         FROM pg_roles WHERE rolname=$1",
    )
    .bind(old_login)
    .fetch_one(pool)
    .await
    .map_err(super::closed)?;
    let mut grants_removed = true;
    for database in [pool, restored] {
        let removed: bool = sqlx::query_scalar(
            "SELECT NOT has_database_privilege($1, current_database(), 'CONNECT')
                AND NOT has_schema_privilege($1, 'public', 'USAGE')
                AND NOT has_schema_privilege($1, 'public', 'CREATE')
                AND NOT EXISTS (
                    SELECT 1
                    FROM pg_catalog.pg_class AS relation
                    JOIN pg_catalog.pg_namespace AS schema ON schema.oid = relation.relnamespace
                    WHERE schema.nspname = 'public'
                      AND CASE WHEN relation.relkind IN ('r','p','v','m','f') THEN
                           has_table_privilege($1, relation.oid, 'SELECT,INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER')
                           OR has_any_column_privilege($1, relation.oid, 'SELECT,INSERT,UPDATE,REFERENCES')
                          WHEN relation.relkind = 'S' THEN
                           has_sequence_privilege($1, relation.oid, 'USAGE,SELECT,UPDATE')
                          ELSE false END
                )",
        )
        .bind(old_login)
        .fetch_one(database)
        .await
        .map_err(super::closed)?;
        grants_removed &= removed;
    }
    Ok((
        closed,
        nologin,
        grants_removed,
        u64::try_from(sessions).map_err(super::closed)?,
        u64::try_from(prepared).map_err(super::closed)?,
    ))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "arquivo de saída da fixture indisponível".to_owned())?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "não foi possível gravar artefato da fixture".to_owned())
}

async fn publish(
    input: PublishInput,
    key: RestoreKey,
    source: &ExternalStore,
    limit: NonZeroUsize,
    deadline: Instant,
) -> Result<(), String> {
    let cut_db = connect(&input.cut.database, "GODMESH_TEST_RESTORE_CUT_URL").await?;
    let restored = connect(&input.restored, "GODMESH_TEST_RESTORE_TARGET_ADMIN_URL").await?;
    let previous = source.observe(&key, deadline).await.map_err(closed)?;
    let registered = source
        .logins(previous.binding(), deadline)
        .await
        .map_err(closed)?;
    if input.old_login != registered.target_login {
        return Err("login indicado para fence diverge da geração anterior".to_owned());
    }
    let generation = previous.binding().generation;
    let target_generation = generation.checked_add(1).ok_or("geração esgotada")?;
    let cut_db_id = allocation_id(&cut_db).await.map_err(closed)?;
    let target_db_id = allocation_id(&restored).await.map_err(closed)?;
    if cut_db_id == target_db_id || cut_db_id != previous.binding().target_db_id {
        return Err("alocação de corte ou restore divergente".to_owned());
    }
    let (admission_closed, nologin_confirmed, grants_removed, remaining_sessions, unknown_effects) =
        check_fence(
            &cut_db,
            &registered.target_login,
            &key,
            generation,
            &restored,
        )
        .await?;
    let fence = Fence {
        key: key.clone(),
        fenced_generation: generation,
        cut_db_id: cut_db_id.clone(),
        target_db_id: target_db_id.clone(),
        old_login: registered.target_login,
        admission_closed,
        nologin_confirmed,
        grants_removed,
        remaining_sessions,
        unknown_effects,
        fenced_at: SystemClock.now_unix(),
    };
    let fence_sha256 = fence.digest().map_err(closed)?;
    let origin_db = connect(&input.origin.database, "GODMESH_TEST_RESTORE_ORIGIN_URL").await?;
    let (origin_identity, origin_service) = actor(&input.origin, origin_db).await?;
    let (_, cut_service) = actor(&input.cut, cut_db).await?;
    let origin = observe_origin(&origin_service, &key, peer(&cut_service), deadline)
        .await
        .map_err(closed)?;
    let cut = observe_cut(
        &cut_service,
        &key,
        generation,
        peer(&origin_service),
        &origin,
        input.backup_prefix,
        deadline,
    )
    .await
    .map_err(closed)?;
    let dump = read_bounded(&input.backup, limit.get())?;
    let mut mark = RestoreHighWaterV1 {
        version: 1,
        key: key.clone(),
        backup_db_id: cut_db_id,
        target_db_id: target_db_id.clone(),
        backup_generation: input.backup_generation,
        fenced_generation: generation,
        target_generation,
        backup_kind: if input.backup_prefix == cut.control.prefix {
            BackupKind::AtCut
        } else {
            BackupKind::BeforeCut
        },
        backup_bytes: u64::try_from(dump.len()).map_err(closed)?,
        backup_sha256: Sha256::digest(&dump).into(),
        backup_prefix: input.backup_prefix,
        cut_prefix: cut.control.prefix,
        backup_proof_count: input.backup_prefix,
        backup_proofs_sha256: cut.backup_proofs_sha256,
        gap_proof_count: cut
            .control
            .prefix
            .checked_sub(input.backup_prefix)
            .ok_or("corte divergente")?,
        gap_proofs_sha256: cut.gap_proofs_sha256,
        control_state_sha256: cut.control.digest().map_err(closed)?,
        blocked_next_seq: cut.control.blocked_next_seq,
        intent_state: cut.control.intent_state,
        attempt_id: cut.control.attempt_id,
        fence_sha256,
        cut_at: cut.cut_at,
        signature: [0; 64],
    };
    let signer = NodeIdentity::load(input.signer).map_err(closed)?;
    mark.sign(&signer).map_err(closed)?;
    let json = serde_json::to_vec(&mark).map_err(closed)?;
    let high = verify_high_water(
        &json,
        limit,
        &ExpectedHighWater {
            key: &key,
            target_db_id: &target_db_id,
            fenced_generation: generation,
            target_generation,
            high_water_sha256: mark.digest().map_err(closed)?,
            fence_sha256,
            harness_public_key: &signer.public_key_hex(),
        },
    )
    .map_err(closed)?;
    if let Some(input_gap) = input.gap {
        let now = SystemClock.now_unix();
        let policy = input_gap.policy(&cut_service, &key);
        let package = capture_gap(
            &origin_service,
            &origin_identity,
            peer(&cut_service),
            &high,
            Window {
                not_before: now,
                expires_at: now + i64::from(input.valid_seconds.get()),
            },
            &policy,
            deadline,
        )
        .await
        .map_err(closed)?;
        write_new(&input_gap.path, &package)?;
    }
    write_new(&input.high_water, &json)?;
    let next = Binding::from_high_water(&high).map_err(closed)?;
    source
        .register_logins(&next, &input.logins.into(), deadline)
        .await
        .map_err(closed)?;
    let observation = source
        .publish(&high, &fence, deadline)
        .await
        .map_err(closed)?;
    source
        .record_origin(observation.binding(), &origin, deadline)
        .await
        .map_err(closed)?;
    Ok(())
}

async fn execute(command: Command) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(u64::from(command.timeout_ms.get()));
    let source =
        ExternalStore::new(connect(&command.control, "GODMESH_TEST_RESTORE_CONTROL_URL").await?);
    match command.action {
        OperatorAction::Seed {
            target_db_id,
            logins,
        } => {
            let binding = Binding {
                key: command.key,
                generation: 1,
                target_db_id,
                high_water_sha256: [0; 32],
                fence_sha256: [0; 32],
            };
            source
                .register_logins(&binding, &logins.into(), deadline)
                .await
                .map_err(closed)?;
            source.seed(&binding, deadline).await.map_err(closed)
        }
        OperatorAction::Publish(input) => {
            publish(*input, command.key, &source, command.limit_bytes, deadline).await
        }
        OperatorAction::ObserveOrigin {
            phase,
            origin,
            destination_public_key,
        } => {
            let observed = source
                .observe(&command.key, deadline)
                .await
                .map_err(closed)?;
            let pool = connect(&origin.database, "GODMESH_TEST_RESTORE_ORIGIN_URL").await?;
            let (_, service) = actor(&origin, pool).await?;
            let peer = SessionPeer {
                node_id: &command.key.local_node,
                public_key: &destination_public_key,
                protocol: 1,
            };
            match phase {
                ObservationPhase::Cas => {
                    let origin = observe_origin(&service, &command.key, peer, deadline)
                        .await
                        .map_err(closed)?;
                    source
                        .record_origin(observed.binding(), &origin, deadline)
                        .await
                        .map_err(closed)?;
                }
                ObservationPhase::Opening => {
                    let origin = observe_opening_origin(&service, &observed, peer, deadline)
                        .await
                        .map_err(closed)?;
                    source
                        .record_opening_origin(&origin, deadline)
                        .await
                        .map_err(closed)?;
                }
            }
            Ok(())
        }
        OperatorAction::SignPromotion {
            high_water,
            expected_reconciliation,
            signer,
            output,
            valid_seconds,
        } => {
            let signer = NodeIdentity::load(signer).map_err(closed)?;
            let observed = source
                .observe(&command.key, deadline)
                .await
                .map_err(closed)?;
            let binding = observed.binding();
            let mark = verify_high_water(
                &read_bounded(&high_water, command.limit_bytes.get())?,
                command.limit_bytes,
                &ExpectedHighWater {
                    key: &command.key,
                    target_db_id: &binding.target_db_id,
                    fenced_generation: binding
                        .generation
                        .checked_sub(1)
                        .ok_or("geração inválida")?,
                    target_generation: binding.generation,
                    high_water_sha256: binding.high_water_sha256,
                    fence_sha256: binding.fence_sha256,
                    harness_public_key: &signer.public_key_hex(),
                },
            )
            .map_err(closed)?;
            let mut digest = [0; 32];
            hex::decode_to_slice(expected_reconciliation, &mut digest).map_err(closed)?;
            let high = mark.high_water();
            let now = SystemClock.now_unix();
            let mut authorization = RestorePromotionV1 {
                version: 1,
                key: command.key,
                target_db_id: high.target_db_id.clone(),
                fenced_generation: high.fenced_generation,
                target_generation: high.target_generation,
                high_water_sha256: mark.digest(),
                reconciliation_sha256: digest,
                fence_sha256: high.fence_sha256,
                not_before: now,
                expires_at: now + i64::from(valid_seconds.get()),
                signature: [0; 64],
            };
            authorization.sign(&signer).map_err(closed)?;
            write_new(
                &output,
                &serde_json::to_vec(&authorization).map_err(closed)?,
            )
        }
    }
}

#[test]
#[ignore = "operador interno da fixture descartável de restore"]
fn process() {
    let path = std::env::var_os("GODMESH_TEST_RESTORE_COMMAND").expect("comando da fixture");
    let bytes = read_bounded(Path::new(&path), CONTEXT_BYTES).expect("ler comando da fixture");
    let command = serde_json::from_slice(&bytes).expect("interpretar comando da fixture");
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(execute(command))
        .expect("operação da fixture de restore");
}
