//! Seleção do banco da geração no executável descartável de testes.

use std::{
    io::Read,
    net::IpAddr,
    num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
};

use godmesh_federation::{
    identity::verify_signature,
    restore_evidence::{
        ExpectedHighWater, ExpectedPromotion, RestoreKey, RestorePromotionV1, verify_high_water,
        verify_promotion,
    },
    snapshot::{SnapshotIdentity, SnapshotLimits, SnapshotPolicy},
};
use godmesh_server::replica_operational::{
    ReplicaService, SessionPeer,
    restore_lab::{
        AdmissionGate, BackupArtifact, ExternalObservation, ExternalStore, LocalRestore,
        allocation_id,
    },
};
use serde::Deserialize;
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use tokio::time::Instant;

use crate::{Clock, SystemClock};

mod operator;

const CONTEXT_BYTES: usize = 32 * 1024;

#[derive(Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct PgAllocation {
    database: String,
    login: String,
    host: IpAddr,
    port: NonZeroU16,
}

impl PgAllocation {
    fn connect_options(&self, url: &str) -> Result<PgConnectOptions, &'static str> {
        let has_password = url
            .strip_prefix("postgresql://")
            .and_then(|url| url.split_once('@'))
            .and_then(|(user_info, _)| user_info.split_once(':'))
            .is_some_and(|(_, password)| !password.is_empty());
        if !self.host.is_loopback() || url.contains('?') || url.contains('#') || !has_password {
            return Err("conexão PostgreSQL fora da fixture de restore");
        }
        let options = url
            .parse::<PgConnectOptions>()
            .map_err(|_| "conexão PostgreSQL inválida para a fixture de restore")?;
        if options.get_host().parse::<IpAddr>().ok() != Some(self.host)
            || options.get_port() != self.port.get()
            || options.get_username() != self.login
            || options.get_database() != Some(&self.database)
            || options.get_socket().is_some()
        {
            return Err("alocação PostgreSQL diverge do contexto de restore");
        }
        Ok(options)
    }

    async fn verify_database(&self, pool: &sqlx::PgPool) -> Result<(), String> {
        let mut tx = pool
            .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .await
            .map_err(|_| "banco da geração de restore indisponível".to_owned())?;
        self.verify_account(&mut tx).await?;
        godmesh_server::migrate::verify_restore_schema(&mut tx)
            .await
            .map_err(|error| error.to_string())?;
        tx.rollback()
            .await
            .map_err(|_| "não foi possível encerrar o preflight de restore".to_owned())
    }

    async fn verify_account(&self, connection: &mut sqlx::PgConnection) -> Result<(), String> {
        let eligible: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM pg_catalog.pg_roles AS role
                 JOIN pg_catalog.pg_database AS db ON db.datname = current_database()
                 WHERE role.rolname = current_user AND session_user = current_user
                   AND role.rolname::text = $1 AND db.datname::text = $2
                   AND host(inet_server_addr()) = $3 AND inet_server_port() = $4
                   AND role.rolcanlogin AND NOT (
                       role.rolsuper OR role.rolcreaterole OR role.rolcreatedb
                       OR role.rolreplication OR role.rolbypassrls
                   )
                   AND db.datdba <> role.oid
                   AND NOT has_schema_privilege(current_user, 'public', 'CREATE')
                   AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_auth_members AS membership
                                   WHERE membership.member = role.oid)
                   AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_class AS relation
                                   JOIN pg_catalog.pg_namespace AS schema
                                     ON schema.oid = relation.relnamespace
                                   WHERE schema.nspname = 'public'
                                     AND relation.relowner = role.oid)
             )",
        )
        .bind(&self.login)
        .bind(&self.database)
        .bind(self.host.to_string())
        .bind(i32::from(self.port.get()))
        .fetch_one(connection)
        .await
        .map_err(|_| "privilégios da geração de restore indisponíveis".to_owned())?;
        if !eligible {
            return Err("login PostgreSQL incompatível com o efetor de restore".to_owned());
        }
        Ok(())
    }

    async fn verify_control_account(&self, pool: &PgPool, consume: bool) -> Result<(), String> {
        let mut connection = pool
            .acquire()
            .await
            .map_err(|_| "fonte de restore indisponível".to_owned())?;
        self.verify_account(&mut connection).await.map_err(|_| {
            "login PostgreSQL incompatível com o papel da fonte de restore".to_owned()
        })?;
        let scoped: bool = sqlx::query_scalar(
            "SELECT NOT has_any_column_privilege('public.federation_restore_generation_logins','INSERT,UPDATE')
               AND NOT has_any_column_privilege('public.federation_restore_control','INSERT')
               AND NOT has_any_column_privilege('public.federation_restore_decision','UPDATE')
               AND NOT has_table_privilege('public.federation_restore_generation_logins','DELETE,TRUNCATE')
               AND NOT has_table_privilege('public.federation_restore_control','DELETE,TRUNCATE')
               AND NOT has_table_privilege('public.federation_restore_decision','DELETE,TRUNCATE')
               AND has_any_column_privilege('public.federation_restore_control','UPDATE') = $1
               AND has_any_column_privilege('public.federation_restore_decision','INSERT') = $1",
        )
        .bind(consume)
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| "permissões da fonte de restore indisponíveis".to_owned())?;
        if !scoped {
            return Err("permissões incompatíveis com a operação na fonte de restore".to_owned());
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RestoreProcess {
    key: RestoreKey,
    generation: NonZeroU64,
    target: PgAllocation,
    target_db_id: String,
    control: PgAllocation,
    control_db_id: String,
    harness_public_key: String,
    action: Action,
    max_evidence_bytes: NonZeroUsize,
    max_backup_bytes: NonZeroUsize,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Initial,
    Retire,
    Reconcile {
        high_water: PathBuf,
        backup: PathBuf,
        gap: Option<SnapshotInput>,
    },
    Consume {
        high_water: PathBuf,
        promotion: PathBuf,
    },
    Serve {
        high_water: PathBuf,
        promotion: PathBuf,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotInput {
    path: PathBuf,
    retention_class: String,
    max_package_bytes: NonZeroUsize,
    max_manifest_bytes: NonZeroU32,
    max_event_bytes: NonZeroU32,
    max_events: NonZeroU32,
}

impl SnapshotInput {
    fn policy(&self, service: &ReplicaService, key: &RestoreKey) -> SnapshotPolicy {
        let origin = &service.config().origin_pin;
        let destination = service.local_pin();
        SnapshotPolicy {
            cluster: key.cluster.clone(),
            tenant_id: key.tenant_id.clone(),
            stream_id: key.stream_id.clone(),
            partition: key.partition,
            authority_epoch: key.authority_epoch,
            origin: SnapshotIdentity {
                node_id: origin.node_id.clone(),
                public_key: origin.public_key.clone(),
            },
            destination: SnapshotIdentity {
                node_id: destination.node_id.clone(),
                public_key: destination.public_key.clone(),
            },
            retention_class: self.retention_class.clone(),
            limits: SnapshotLimits {
                max_package_bytes: self.max_package_bytes,
                max_manifest_bytes: self.max_manifest_bytes,
                max_event_bytes: self.max_event_bytes,
                max_events: self.max_events,
            },
        }
    }
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut result = Vec::new();
    let probe = u64::try_from(limit)
        .ok()
        .and_then(|limit| limit.checked_add(1))
        .ok_or("limite de arquivo inválido no contexto de restore")?;
    std::fs::File::open(path)
        .and_then(|file| file.take(probe).read_to_end(&mut result))
        .map_err(|_| "arquivo de restore da fixture indisponível".to_owned())?;
    if result.len() > limit {
        return Err("arquivo da fixture excede o limite de restore".to_owned());
    }
    Ok(result)
}

fn closed(error: impl std::fmt::Debug) -> String {
    format!("restore_history_incomplete: {error:?}")
}

impl RestoreProcess {
    fn from_env() -> Result<Self, String> {
        let path = std::env::var_os("GODMESH_TEST_RESTORE_CONTEXT")
            .ok_or("restore de laboratório exige seleção externa")?;
        let bytes = read_bounded(Path::new(&path), CONTEXT_BYTES)?;
        let context: Self = serde_json::from_slice(&bytes)
            .map_err(|_| "seleção externa de restore inválida".to_owned())?;
        if context.control_db_id == context.target_db_id || context.control == context.target {
            return Err("a fonte de restore deve ser externa ao banco alvo".to_owned());
        }
        Ok(context)
    }

    pub(super) async fn connect_target(&self, url: &str) -> Result<PgPool, String> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect_with(self.target.connect_options(url)?)
            .await
            .map_err(|_| "login do efetor de restore recusado".to_owned())?;
        if allocation_id(&pool).await.map_err(closed)? != self.target_db_id {
            return Err("alocação física do banco de restore divergente".to_owned());
        }
        self.target.verify_database(&pool).await?;
        Ok(pool)
    }

    async fn external(
        &self,
        deadline: Instant,
    ) -> Result<(ExternalStore, ExternalObservation), String> {
        let url = std::env::var("GODMESH_TEST_RESTORE_CONTROL_URL")
            .map_err(|_| "fonte de restore ausente".to_owned())?;
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(self.control.connect_options(&url)?)
            .await
            .map_err(|_| "fonte de restore indisponível".to_owned())?;
        let consume = matches!(self.action, Action::Consume { .. });
        self.control.verify_control_account(&pool, consume).await?;
        let id = allocation_id(&pool).await.map_err(closed)?;
        if id != self.control_db_id {
            return Err("alocação da fonte externa de restore divergente".to_owned());
        }
        let source = ExternalStore::new(pool);
        let observed = if consume {
            // Depois de um COMMIT incerto, aguarde a linha em vez de observar
            // uma fotografia anterior à conclusão da tentativa.
            source.resolve_promotion(&self.key, deadline).await
        } else {
            source.observe(&self.key, deadline).await
        }
        .map_err(closed)?;
        if observed.binding().generation != self.generation.get()
            || observed.binding().target_db_id != self.target_db_id
        {
            return Err("a geração de restore selecionada foi superada".to_owned());
        }
        let roles = source
            .logins(observed.binding(), deadline)
            .await
            .map_err(closed)?;
        let expected_control = if consume {
            roles.cas_login
        } else {
            roles.observer_login
        };
        if self.target.login != roles.target_login || self.control.login != expected_control {
            return Err("logins divergem do registro da geração de restore".to_owned());
        }
        Ok((source, observed))
    }

    pub(super) async fn prepare(
        &self,
        replica: ReplicaService,
    ) -> Result<Option<Arc<ReplicaService>>, String> {
        let deadline = Instant::now() + replica.config().attempt_timeout;
        let (source, observed) = self.external(deadline).await?;
        let binding = observed.binding();
        if let (Action::Consume { promotion, .. }, Some(digest)) =
            (&self.action, observed.authorization_sha256())
        {
            let bytes = read_bounded(promotion, self.max_evidence_bytes.get())?;
            if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{') {
                return Err("autorização de restore malformada".to_owned());
            }
            let recorded: RestorePromotionV1 = serde_json::from_slice(&bytes).map_err(closed)?;
            if recorded.digest().map_err(closed)? != digest {
                return Err("outra autorização já foi consumida nesta geração".to_owned());
            }
            verify_signature(
                &self.harness_public_key,
                &recorded.canonical_bytes().map_err(closed)?,
                &hex::encode(recorded.signature),
            )
            .map_err(closed)?;
            println!(
                "event=restore_promotion_replayed generation={}",
                binding.generation
            );
            return Ok(None);
        }
        let gate = AdmissionGate::new(binding.clone()).map_err(closed)?;
        let service = Arc::new(replica.with_restore_gate(gate.clone()));
        let local = LocalRestore::new(Arc::clone(&service), gate).map_err(closed)?;
        let high_water = match &self.action {
            Action::Initial => {
                local
                    .boot_initial(&observed, deadline)
                    .await
                    .map_err(closed)?;
                return Ok(Some(service));
            }
            Action::Retire => {
                local.retire(deadline).await.map_err(closed)?;
                println!("event=restore_retired generation={}", binding.generation);
                return Ok(None);
            }
            Action::Reconcile { high_water, .. }
            | Action::Consume { high_water, .. }
            | Action::Serve { high_water, .. } => high_water,
        };
        local
            .boot_closed(&observed, deadline)
            .await
            .map_err(closed)?;
        let mark = verify_high_water(
            &read_bounded(high_water, self.max_evidence_bytes.get())?,
            self.max_evidence_bytes,
            &ExpectedHighWater {
                key: &self.key,
                target_db_id: &self.target_db_id,
                fenced_generation: binding.generation - 1,
                target_generation: binding.generation,
                high_water_sha256: binding.high_water_sha256,
                fence_sha256: binding.fence_sha256,
                harness_public_key: &self.harness_public_key,
            },
        )
        .map_err(closed)?;
        let pin = &service.config().origin_pin;
        let peer = SessionPeer {
            node_id: &pin.node_id,
            public_key: &pin.public_key,
            protocol: 1,
        };
        let origin = if matches!(self.action, Action::Serve { .. }) {
            observed.opening_origin()
        } else {
            observed.origin()
        }
        .ok_or("restore_history_incomplete: observação de A ausente")?;
        match &self.action {
            Action::Reconcile { backup, gap, .. } => {
                let bytes = read_bounded(backup, self.max_backup_bytes.get())?;
                mark.verify_backup(&bytes).map_err(closed)?;
                let snapshot = gap
                    .as_ref()
                    .map(|input| {
                        read_bounded(&input.path, input.max_package_bytes.get())
                            .map(|data| (data, input.policy(&service, &self.key)))
                    })
                    .transpose()?;
                let reconciliation = local
                    .reconcile(
                        peer,
                        &mark,
                        &observed,
                        origin,
                        BackupArtifact {
                            bytes: u64::try_from(bytes.len()).map_err(closed)?,
                            sha256: mark.high_water().backup_sha256,
                        },
                        snapshot
                            .as_ref()
                            .map(|(data, policy)| (data.as_slice(), policy)),
                        deadline,
                    )
                    .await
                    .map_err(closed)?;
                println!(
                    "event=restore_reconciled generation={} digest={}",
                    binding.generation,
                    hex::encode(reconciliation.digest().map_err(closed)?)
                );
                Ok(None)
            }
            Action::Consume { promotion, .. } | Action::Serve { promotion, .. } => {
                let consume = matches!(self.action, Action::Consume { .. });
                let candidate = if consume {
                    local
                        .promotion_candidate(peer, &mark, origin, deadline)
                        .await
                } else {
                    local
                        .opening_candidate(peer, &mark, &observed, deadline)
                        .await
                }
                .map_err(closed)?;
                let verified = verify_promotion(
                    &read_bounded(promotion, self.max_evidence_bytes.get())?,
                    self.max_evidence_bytes,
                    &ExpectedPromotion {
                        high_water: &mark,
                        reconciliation: candidate.reconciliation(),
                    },
                    SystemClock.now_unix(),
                )
                .map_err(closed)?;
                if consume {
                    local
                        .prepare_promotion(peer, &mark, &verified, origin, deadline)
                        .await
                        .map_err(closed)?;
                    local
                        .consume_promotion(peer, &mark, &source, &verified, origin, deadline)
                        .await
                        .map_err(closed)?;
                    println!(
                        "event=restore_promotion_consumed generation={}",
                        binding.generation
                    );
                    return Ok(None);
                }
                local
                    .open(peer, &mark, &observed, &verified, origin, deadline)
                    .await
                    .map_err(closed)?;
                Ok(Some(service))
            }
            Action::Initial | Action::Retire => unreachable!("tratado antes da alta marca"),
        }
    }
}

#[test]
#[ignore = "executado em processo separado pela fixture A/B/C de restore"]
fn process() {
    let mut config = super::Config::from_env(super::Args::Serve).expect("configuração do restore");
    config.restore_process = Some(RestoreProcess::from_env().expect("seleção de restore"));
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime da fixture")
        .block_on(super::run(config))
        .expect("efetor da fixture de restore");
}

#[cfg(test)]
mod tests {
    use super::PgAllocation;

    const DSN: &str = "postgresql://restore_b2:fixture-password@127.0.0.1:5433/restore_target";

    #[test]
    fn restore_conecta_apenas_ao_login_e_banco_selecionados() {
        let selection: PgAllocation = serde_json::from_value(serde_json::json!({
            "database": "restore_target",
            "login": "restore_b2",
            "host": "127.0.0.1",
            "port": 5433,
        }))
        .unwrap();
        assert!(selection.connect_options(DSN).is_ok());
        for other in [
            DSN.replace("5433", "5434"),
            DSN.replace("restore_b2", "restore_b1"),
            DSN.replace("restore_target", "restore_c"),
            DSN.replace("127.0.0.1", "192.0.2.1"),
            DSN.replace(":fixture-password", ""),
            format!("{DSN}?options=-csearch_path%3Dprivate"),
        ] {
            assert!(selection.connect_options(&other).is_err());
        }
    }

    #[tokio::test]
    async fn restore_recusa_usar_o_login_administrativo_como_efetor() {
        let Ok(test_dir) = std::env::var("GODMESH_TEST_TEMP") else {
            eprintln!("este teste exige a instância PostgreSQL descartável do harness");
            return;
        };
        assert!(
            std::path::Path::new(&test_dir)
                .join("pg/PG_VERSION")
                .is_file()
        );
        let pool = sqlx::PgPool::connect(&std::env::var("GODMESH_TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let port = sqlx::query_scalar::<_, i32>("SELECT inet_server_port()")
            .fetch_one(&pool)
            .await
            .unwrap();
        let allocation = PgAllocation {
            database: "postgres".to_owned(),
            login: "gm_m1c".to_owned(),
            host: "127.0.0.1".parse().unwrap(),
            port: u16::try_from(port).unwrap().try_into().unwrap(),
        };
        assert_eq!(
            allocation.verify_database(&pool).await.unwrap_err(),
            "login PostgreSQL incompatível com o efetor de restore"
        );
    }
}
