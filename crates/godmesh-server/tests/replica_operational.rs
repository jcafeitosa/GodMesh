mod common;

use godmesh_federation::NodeIdentity;
use godmesh_server::replica_operational::{ReplicaConfig, Role};

use godmesh_server as server;
#[path = "common/replica_fixture.rs"]
mod fixture;
use fixture::{CLUSTER, ReceiverFixture, boot_source, deadline, env, isolated_database};

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

use godmesh_server::{
    portable_join::PortableJoin,
    replica_operational::{
        EffectError, PreflightError, ReceiverState, ReplicaService, SessionPeer,
    },
    replication::{Error as ReplicaError, Limits},
};
use tokio::time::Instant;

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
    let Some(f) = ReceiverFixture::new("export").await else {
        return;
    };
    let actual = [b" \t".to_vec(), f.frame(1), b"\n ".to_vec()].concat();
    let limits = Limits::new(8, 2048, 4096).unwrap();
    let service = boot_source(
        &f,
        Arc::new(NodeIdentity::generate_in_memory().unwrap()),
        std::slice::from_ref(&actual),
    )
    .await;
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

#[test]
fn cota_de_streams_respeita_o_limite_do_semaforo() {
    use tokio::sync::Semaphore;

    let mut vars = env("receiver");
    let (_, value) = vars
        .iter_mut()
        .find(|(name, _)| name == "GODMESH_FEDERATION_M4_MAX_STREAMS_PER_CONNECTION")
        .unwrap();
    *value = Semaphore::MAX_PERMITS.to_string();
    let at_limit = ReplicaConfig::from_env_with(vars.clone()).unwrap().unwrap();
    assert_eq!(
        at_limit.max_streams_per_connection.get(),
        Semaphore::MAX_PERMITS
    );
    let (_, value) = vars
        .iter_mut()
        .find(|(name, _)| name == "GODMESH_FEDERATION_M4_MAX_STREAMS_PER_CONNECTION")
        .unwrap();
    *value = (Semaphore::MAX_PERMITS + 1).to_string();
    let error = ReplicaConfig::from_env_with(vars).unwrap_err();
    assert_eq!(error.field, "MAX_STREAMS_PER_CONNECTION");
}

#[tokio::test]
async fn admissoes_canceladas_preservam_pool_e_permit_retem_lock() {
    use godmesh_federation::node::{LinkAuthorizationError, LinkIdentity};
    use godmesh_server::replica_operational::authorize_link_until;

    let Some(f) = ReceiverFixture::new("link_lease").await else {
        return;
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*f.pool.connect_options()).clone())
        .await
        .unwrap();
    let portable = Arc::new(PortableJoin::new(
        f.portable_config.clone(),
        pool.clone(),
        f.receiver.clone(),
    ));
    let identity = |node: &NodeIdentity| LinkIdentity {
        node_id: node.node_id().to_owned(),
        public_key: node.public_key_bytes(),
        cluster: CLUSTER.into(),
    };
    let local = identity(&f.receiver);
    let remote = identity(&f.origin);
    let mut external = f.pool.begin().await.unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *external)
        .await
        .unwrap();
    f.portable.lock_cluster_guard(&mut external).await.unwrap();
    let mut cancelled = None;
    for _ in 0..4 {
        let waiter: i32 = tokio::time::timeout(
            Duration::from_secs(1),
            sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&pool),
        )
        .await
        .unwrap()
        .unwrap();
        assert_ne!(
            cancelled,
            Some(waiter),
            "uma sessão cancelada foi devolvida ao pool"
        );
        cancelled = Some(waiter);
        let attempt = authorize_link_until(portable.clone(), None, &local, &remote, deadline());
        tokio::pin!(attempt);
        tokio::select! {
            result = &mut attempt => panic!("o lock externo ainda está retido: {:?}", result.err()),
            () = espera_lock_do_enlace(&f.pool,blocker,waiter) => {}
        };
        // Encerra o future externamente enquanto aguarda o advisory lock.
        // O pool precisa servir outra operação enquanto o bloqueador segue ativo.
    }
    let session: (String, String, i64) = tokio::time::timeout(
        Duration::from_secs(1),
        sqlx::query_as(
            "SELECT current_setting('lock_timeout'),current_setting('statement_timeout'),
            (SELECT count(*) FROM pg_locks WHERE pid=pg_backend_pid() AND locktype='advisory')",
        )
        .fetch_one(&pool),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(session, ("0".into(), "0".into(), 0));
    let timed = authorize_link_until(
        portable.clone(),
        None,
        &local,
        &remote,
        Instant::now() + Duration::from_millis(80),
    )
    .await;
    assert!(matches!(timed, Err(LinkAuthorizationError::Timeout)));
    external.rollback().await.unwrap();

    let (token, expires) = authorize_link_until(portable, None, &local, &remote, deadline())
        .await
        .unwrap_or_else(|err| panic!("{err:?}"));
    assert_eq!(expires, None);
    let lock: String = format!("godmesh.federation.portable.v2:{CLUSTER}");
    let mut contender = f.pool.begin().await.unwrap();
    let acquired = |lock: String| {
        sqlx::query_scalar::<_, bool>("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
            .bind(lock)
    };
    assert!(
        !acquired(lock.clone())
            .fetch_one(&mut *contender)
            .await
            .unwrap()
    );
    assert!(pool.try_acquire().is_none());
    drop(token);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if acquired(lock.clone())
                .fetch_one(&mut *contender)
                .await
                .unwrap()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    contender.rollback().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

async fn espera_lock_do_enlace(pool: &sqlx::PgPool, blocker: i32, waiter: i32) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS (
                SELECT FROM pg_locks WHERE locktype='advisory' AND NOT granted AND $1 = ANY(pg_blocking_pids(pid)) AND pid=$2
            )").bind(blocker).bind(waiter).fetch_one(pool).await.unwrap();
            if waiting { return; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
}
