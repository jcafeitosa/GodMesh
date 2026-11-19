use crate as server;
use godmesh_federation::snapshot::{SnapshotIdentity, SnapshotLimits};
use std::{
    num::{NonZeroU32, NonZeroUsize},
    sync::Arc,
};

use super::*;
#[path = "../../../tests/common/replica_fixture.rs"]
mod fixture;
use fixture::{CLASS, CLUSTER, ReceiverFixture, boot_source, deadline};

const WINDOW: Window = Window {
    not_before: 105,
    expires_at: 250,
};

fn identity(id: &NodeIdentity) -> SnapshotIdentity {
    SnapshotIdentity {
        node_id: id.node_id().to_owned(),
        public_key: id.public_key_hex(),
    }
}

fn peer(id: &SnapshotIdentity) -> SessionPeer<'_> {
    SessionPeer {
        node_id: &id.node_id,
        public_key: &id.public_key,
        protocol: 1,
    }
}

struct Three {
    a: ReceiverFixture,
    b: ReceiverFixture,
    c: ReceiverFixture,
    source: ReplicaService,
    frames: Vec<Vec<u8>>,
}

impl Three {
    async fn new(suffix: &str) -> Option<Self> {
        let a = ReceiverFixture::new(&format!("{suffix}_a")).await?;
        let c_node = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let tenant = &a.config.namespace.tenant_id;
        let b = ReceiverFixture::new_with(
            &format!("{suffix}_b"),
            a.origin.clone(),
            a.receiver.clone(),
            tenant,
        )
        .await
        .unwrap();
        let c = ReceiverFixture::new_with(
            &format!("{suffix}_c"),
            a.origin.clone(),
            c_node.clone(),
            tenant,
        )
        .await
        .unwrap();
        b.reserve().await;
        c.reserve().await;
        for dest in [&b, &c] {
            sqlx::query(
                "INSERT INTO federation_snapshot_receipt_policy
                VALUES ('test_receipt',1,$1,$2,$3,$4,0,7,'BR','snapshot_receipt',20,true)",
            )
            .bind(dest.receiver.node_id())
            .bind(dest.origin.node_id())
            .bind(tenant)
            .bind(&dest.config.namespace.stream_id)
            .execute(&dest.pool)
            .await
            .unwrap();
        }
        for dest in [&b, &c] {
            sqlx::query(
                "INSERT INTO federation_snapshot_intent_policy
                VALUES ('test_intent',1,$1,$2,$3,$4,0,7,'BR','snapshot_intent',10,true,true)",
            )
            .bind(dest.receiver.node_id())
            .bind(dest.origin.node_id())
            .bind(tenant)
            .bind(&dest.config.namespace.stream_id)
            .execute(&dest.pool)
            .await
            .unwrap();
        }
        let frames: Vec<_> = (1..=3)
            .map(|seq| [b" \t".to_vec(), a.frame(seq), b"\n".to_vec()].concat())
            .collect();
        let source = boot_source(&a, c_node, &frames).await;
        Some(Self {
            a,
            b,
            c,
            source,
            frames,
        })
    }

    fn policy(&self, dest: &ReceiverFixture) -> SnapshotPolicy {
        SnapshotPolicy {
            cluster: CLUSTER.into(),
            origin: identity(&self.a.origin),
            destination: identity(&dest.receiver),
            tenant_id: self.a.config.namespace.tenant_id.clone(),
            stream_id: self.a.config.namespace.stream_id.clone(),
            partition: 0,
            authority_epoch: 7,
            retention_class: CLASS.into(),
            limits: SnapshotLimits {
                max_events: NonZeroU32::new(10).unwrap(),
                max_event_bytes: NonZeroU32::new(4096).unwrap(),
                max_manifest_bytes: NonZeroU32::new(4096).unwrap(),
                max_package_bytes: NonZeroUsize::new(16384).unwrap(),
            },
        }
    }

    async fn capture(
        &self,
        policy: &SnapshotPolicy,
        base: i64,
    ) -> Result<Vec<u8>, SnapshotEffectError> {
        capture(
            &self.source,
            &self.a.origin,
            peer(&policy.destination),
            base,
            WINDOW,
            policy,
            deadline(),
        )
        .await
    }
}

async fn effects(fx: &ReceiverFixture) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM event_position),
        (SELECT count(*) FROM federation_replica_proof),
        (SELECT prefix FROM federation_replica_checkpoint),
        (SELECT count(*) FROM federation_snapshot_receipt)",
    )
    .fetch_one(&fx.pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn tres_bancos_commit_atomico_replay_exato_e_base_positiva() {
    let Some(f) = Three::new("snapshot_commit").await else {
        return;
    };
    let b = eligible_b2a(f.b.boot().await.unwrap());
    let c = eligible_b2a(f.c.boot().await.unwrap());
    let policy = f.policy(&f.b);
    let p = f.capture(&policy, 0).await.unwrap();
    let expected = PersistedLocal {
        first: 1,
        through: 3,
        replay: false,
    };
    assert_eq!(
        snapshot::verify_snapshot(&p, &policy).unwrap().events(),
        f.frames
    );

    // Falha no último INSERT, depois das escritas A1, faz rollback de tudo.
    sqlx::query("ALTER TABLE federation_snapshot_receipt ADD CONSTRAINT fixture_cut_failure CHECK (cut_seq<>3)")
        .execute(&f.b.pool).await.unwrap();
    let failed = b
        .apply_b2a(peer(&policy.origin), &p, &policy, deadline())
        .await;
    assert!(
        matches!(
            failed,
            Err(SnapshotEffectError::Effect(EffectError::Storage(_)))
        ),
        "{failed:?}"
    );
    assert_eq!(effects(&f.b).await, (0, 0, 0, 0));
    sqlx::query("ALTER TABLE federation_snapshot_receipt DROP CONSTRAINT fixture_cut_failure")
        .execute(&f.b.pool)
        .await
        .unwrap();

    assert_eq!(
        b.apply_b2a(peer(&policy.origin), &p, &policy, deadline())
            .await
            .unwrap(),
        expected
    );
    assert_eq!(effects(&f.b).await, (3, 3, 3, 1));
    assert_eq!(
        b.apply_b2a(peer(&policy.origin), &p, &policy, deadline())
            .await
            .unwrap(),
        PersistedLocal {
            replay: true,
            ..expected
        }
    );
    assert_eq!(effects(&f.c).await, (0, 0, 0, 0));
    let stored: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT frame FROM federation_replica_proof ORDER BY seq")
            .fetch_all(&f.b.pool)
            .await
            .unwrap();
    assert_eq!(stored, f.frames);
    assert_eq!(effects(&f.a).await, (3, 3, 3, 0));

    let c_policy = f.policy(&f.c);
    c.0.apply_copy(
        peer(&c_policy.origin),
        Limits::new(3, 4096, 8192).unwrap(),
        &f.frames[..1],
        deadline(),
    )
    .await
    .unwrap();
    let c_package = f.capture(&c_policy, 1).await.unwrap();
    assert_eq!(
        c.apply_b2a(peer(&c_policy.origin), &c_package, &c_policy, deadline())
            .await
            .unwrap(),
        PersistedLocal {
            first: 2,
            through: 3,
            replay: false
        }
    );
    assert_eq!(effects(&f.c).await, (3, 3, 3, 1));
    let b_hash: Vec<u8> =
        sqlx::query_scalar("SELECT package_digest FROM federation_snapshot_receipt")
            .fetch_one(&f.b.pool)
            .await
            .unwrap();
    assert_eq!(b_hash, Sha256::digest(&p).as_slice());
}

#[tokio::test]
async fn limites_exatos_e_primeira_falha_por_posicao() {
    let Some(f) = Three::new("snapshot_bounds").await else {
        return;
    };
    let mut policy = f.policy(&f.b);
    let raw = f.capture(&policy, 0).await.unwrap();
    let manifest_bytes = u32::from_be_bytes(raw[..4].try_into().unwrap());
    policy.limits.max_manifest_bytes = NonZeroU32::new(manifest_bytes).unwrap();
    let max_proof = f.frames.iter().map(Vec::len).max().unwrap() as u32;
    policy.limits.max_event_bytes = NonZeroU32::new(max_proof).unwrap();
    assert_eq!(f.capture(&policy, 0).await.unwrap(), raw);
    policy.limits.max_manifest_bytes = NonZeroU32::new(manifest_bytes - 1).unwrap();
    assert!(matches!(
        f.capture(&policy, 0).await,
        Err(SnapshotEffectError::LimitExceeded)
    ));
    policy.limits.max_manifest_bytes = NonZeroU32::new(manifest_bytes).unwrap();
    policy.limits.max_event_bytes = NonZeroU32::new(max_proof - 1).unwrap();
    assert!(matches!(
        f.capture(&policy, 0).await,
        Err(SnapshotEffectError::LimitExceeded)
    ));
    policy.limits.max_event_bytes = NonZeroU32::new(max_proof).unwrap();
    policy.limits.max_package_bytes = NonZeroUsize::new(raw.len()).unwrap();
    assert_eq!(f.capture(&policy, 0).await.unwrap(), raw);
    policy.limits.max_package_bytes = NonZeroUsize::new(raw.len() - 1).unwrap();
    assert!(matches!(
        f.capture(&policy, 0).await,
        Err(SnapshotEffectError::LimitExceeded)
    ));
    policy.limits.max_package_bytes = NonZeroUsize::new(raw.len()).unwrap();
    policy.limits.max_events = NonZeroU32::new(1).unwrap();
    assert!(matches!(
        f.capture(&policy, 0).await,
        Err(SnapshotEffectError::LimitExceeded)
    ));
    sqlx::query("DELETE FROM federation_replica_proof WHERE seq=3")
        .execute(&f.a.pool)
        .await
        .unwrap();
    // O limite em 2 precede o defeito em 3.
    assert!(matches!(
        f.capture(&policy, 0).await,
        Err(SnapshotEffectError::LimitExceeded)
    ));
    sqlx::query("DELETE FROM federation_replica_proof WHERE seq=2")
        .execute(&f.a.pool)
        .await
        .unwrap();
    // Na própria posição 2, a falta de prova precede a cota simultânea.
    let result = f.capture(&policy, 0).await;
    assert!(
        matches!(
            result,
            Err(SnapshotEffectError::Effect(EffectError::Replication(
                ReplicaError::ProofMissing
            )))
        ),
        "{result:?}"
    );
    let page = f
        .source
        .prepare_copy(
            peer(&policy.destination),
            0,
            Limits::new(1, 4096, 8192).unwrap(),
            deadline(),
            Ok,
        )
        .await
        .unwrap();
    assert_eq!(page.frames, f.frames[..1]);
    assert!(page.has_more);
}

#[tokio::test]
async fn replay_distingue_conflito_ausencia_expiracao_e_suspensao() {
    use std::sync::atomic::{AtomicI64, Ordering};
    let Some(f) = Three::new("snapshot_replay").await else {
        return;
    };
    let clock = Arc::new(AtomicI64::new(110));
    let b = eligible_b2a(f.b.boot().await.unwrap().with_clock({
        let clock = clock.clone();
        Arc::new(move || clock.load(Ordering::SeqCst))
    }));
    let policy = f.policy(&f.b);
    let source = peer(&policy.origin);
    let raw = f.capture(&policy, 0).await.unwrap();
    let alternate = f.capture(&policy, 1).await.unwrap();
    b.0.apply_copy(
        source,
        Limits::new(3, 4096, 8192).unwrap(),
        &f.frames[..1],
        deadline(),
    )
    .await
    .unwrap();
    assert!(matches!(
        b.apply_b2a(source, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::CheckpointMismatch)
    ));
    b.0.apply_copy(
        source,
        Limits::new(3, 4096, 8192).unwrap(),
        &f.frames[1..],
        deadline(),
    )
    .await
    .unwrap();
    assert!(matches!(
        b.apply_b2a(source, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::ReceiptMissing)
    ));

    let c_policy = f.policy(&f.c);
    let c_source = peer(&c_policy.origin);
    let c_raw = f.capture(&c_policy, 0).await.unwrap();
    let c_alt = f.capture(&c_policy, 1).await.unwrap();
    let c = eligible_b2a(f.c.boot().await.unwrap().with_clock({
        let clock = clock.clone();
        Arc::new(move || clock.load(Ordering::SeqCst))
    }));
    c.apply_b2a(c_source, &c_raw, &c_policy, deadline())
        .await
        .unwrap();
    sqlx::query("UPDATE federation_snapshot_receipt_policy SET active=false")
        .execute(&f.c.pool)
        .await
        .unwrap();
    assert!(matches!(
        c.apply_b2a(c_source, &c_raw, &c_policy, deadline()).await,
        Err(SnapshotEffectError::ReceiptPolicy)
    ));
    sqlx::query("INSERT INTO federation_snapshot_receipt_policy
        SELECT policy_id,2,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,
            jurisdiction,purpose,500,true FROM federation_snapshot_receipt_policy WHERE policy_version=1")
        .execute(&f.c.pool).await.unwrap();
    clock.store(120, Ordering::SeqCst);
    assert!(
        c.apply_b2a(c_source, &c_raw, &c_policy, deadline())
            .await
            .unwrap()
            .replay
    );
    let times: (i64, i64, i64) = sqlx::query_as(
        "SELECT applied_at,expires_at,policy_version FROM federation_snapshot_receipt",
    )
    .fetch_one(&f.c.pool)
    .await
    .unwrap();
    assert_eq!(times, (110, 130, 1));
    sqlx::query("UPDATE federation_snapshot_receipt SET expires_at=131")
        .execute(&f.c.pool)
        .await
        .unwrap();
    assert!(matches!(
        c.apply_b2a(c_source, &c_raw, &c_policy, deadline()).await,
        Err(SnapshotEffectError::ReceiptInconsistent)
    ));
    sqlx::query("UPDATE federation_snapshot_receipt SET expires_at=130")
        .execute(&f.c.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE federation_replica_proof SET frame=decode('20','hex') || frame WHERE seq=2",
    )
    .execute(&f.c.pool)
    .await
    .unwrap();
    assert!(matches!(
        c.apply_b2a(c_source, &c_raw, &c_policy, deadline()).await,
        Err(SnapshotEffectError::SnapshotConflict)
    ));
    sqlx::query("UPDATE federation_replica_proof SET frame=$1 WHERE seq=2")
        .bind(&f.frames[1])
        .execute(&f.c.pool)
        .await
        .unwrap();
    clock.store(130, Ordering::SeqCst);
    assert!(matches!(
        c.apply_b2a(c_source, &c_alt, &c_policy, deadline()).await,
        Err(SnapshotEffectError::SnapshotConflict)
    ));
    assert!(matches!(
        c.apply_b2a(c_source, &c_raw, &c_policy, deadline()).await,
        Err(SnapshotEffectError::ReceiptExpired)
    ));
    clock.store(125, Ordering::SeqCst);
    let delete =
        sqlx::query("DELETE FROM federation_snapshot_receipt_policy WHERE policy_version=1")
            .execute(&f.c.pool)
            .await
            .unwrap_err();
    assert_eq!(
        delete.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    assert!(
        c.apply_b2a(c_source, &c_raw, &c_policy, deadline())
            .await
            .unwrap()
            .replay
    );
    c.0.suspend_history(c_source, 3, deadline()).await.unwrap();
    assert!(matches!(
        c.apply_b2a(c_source, &c_raw, &c_policy, deadline()).await,
        Err(SnapshotEffectError::Effect(EffectError::SuspendedHistory {
            blocked_next_seq: 4
        }))
    ));
    assert_eq!(effects(&f.c).await, (3, 3, 3, 1));

    assert!(matches!(
        b.apply_b2a(source, &alternate, &policy, deadline()).await,
        Err(SnapshotEffectError::ReceiptMissing)
    ));
    assert_eq!(effects(&f.b).await, (3, 3, 3, 0));
}

async fn wait_on(pool: &sqlx::PgPool, blocker: i32) -> i32 {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            if let Some(pid) = sqlx::query_scalar(
                "SELECT pid FROM pg_locks WHERE NOT granted AND $1=ANY(pg_blocking_pids(pid)) LIMIT 1",
            ).bind(blocker).fetch_optional(pool).await.unwrap() { return pid; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap()
}

#[tokio::test]
async fn barreiras_de_revogacao_e_prazo_preservam_atomicidade() {
    let Some(f) = Three::new("snapshot_order").await else {
        return;
    };
    let policy_b = f.policy(&f.b);
    let raw_b = f.capture(&policy_b, 0).await.unwrap();
    let b = eligible_b2a(f.b.boot().await.unwrap());
    let mut earlier = f.b.pool.begin().await.unwrap();
    let earlier_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *earlier)
        .await
        .unwrap();
    sqlx::query("UPDATE federation_replica_acl SET revoked=true WHERE direction='import'")
        .execute(&mut *earlier)
        .await
        .unwrap();
    let attempt = b.apply_b2a(peer(&policy_b.origin), &raw_b, &policy_b, deadline());
    tokio::pin!(attempt);
    tokio::select! {
        outcome = &mut attempt => panic!("aplicação não aguardou ACL: {outcome:?}"),
        _ = wait_on(&f.b.pool,earlier_pid) => {}
    }
    earlier.commit().await.unwrap();
    assert!(matches!(
        attempt.await,
        Err(SnapshotEffectError::Effect(EffectError::Replication(
            ReplicaError::AclDenied
        )))
    ));
    assert_eq!(effects(&f.b).await, (0, 0, 0, 0));

    let policy_c = f.policy(&f.c);
    let raw_c = f.capture(&policy_c, 0).await.unwrap();
    let c = eligible_b2a(f.c.boot().await.unwrap());
    let mut gate = f.c.pool.begin().await.unwrap();
    let gate_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    sqlx::query("LOCK TABLE federation_snapshot_receipt IN SHARE MODE")
        .execute(&mut *gate)
        .await
        .unwrap();
    let timeout = c
        .apply_b2a(
            peer(&policy_c.origin),
            &raw_c,
            &policy_c,
            Instant::now() + std::time::Duration::from_millis(90),
        )
        .await;
    assert!(
        matches!(
            timeout,
            Err(SnapshotEffectError::Effect(EffectError::Timeout))
        ),
        "{timeout:?}"
    );
    assert_eq!(effects(&f.c).await, (0, 0, 0, 0));

    let applying = c.apply_b2a(peer(&policy_c.origin), &raw_c, &policy_c, deadline());
    tokio::pin!(applying);
    let writer = tokio::select! {
        outcome = &mut applying => panic!("aplicação não aguardou INSERT: {outcome:?}"),
        pid = wait_on(&f.c.pool,gate_pid) => pid,
    };
    let revoking =
        sqlx::query("UPDATE federation_replica_acl SET revoked=true WHERE direction='import'")
            .execute(&f.c.pool);
    tokio::pin!(revoking);
    tokio::select! {
        result = &mut revoking => panic!("revogação passou à frente: {result:?}"),
        _ = wait_on(&f.c.pool,writer) => {}
    }
    gate.commit().await.unwrap();
    applying.await.unwrap();
    revoking.await.unwrap();
    assert_eq!(effects(&f.c).await, (3, 3, 3, 1));
    assert!(matches!(
        c.apply_b2a(peer(&policy_c.origin), &raw_c, &policy_c, deadline())
            .await,
        Err(SnapshotEffectError::Effect(EffectError::Replication(
            ReplicaError::AclDenied
        )))
    ));
}

#[test]
fn ttl_considera_tres_tetos_e_overflow() {
    assert!(matches!(
        SnapshotEffectError::from(SnapshotError::ScopeMismatch),
        SnapshotEffectError::Codec(SnapshotError::ScopeMismatch)
    ));
    assert_eq!(receipt_expiry(10, 20, 50, 80).unwrap(), 30);
    assert_eq!(receipt_expiry(10, 90, 50, 80).unwrap(), 50);
    assert_eq!(receipt_expiry(10, 90, 100, 80).unwrap(), 80);
    assert!(matches!(
        receipt_expiry(i64::MAX - 1, 2, i64::MAX, i64::MAX),
        Err(SnapshotEffectError::ReceiptInconsistent)
    ));
}

#[tokio::test]
async fn verificacao_b1_que_consumiu_o_prazo_nao_abre_transacao() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let Some(f) = Three::new("snapshot_verify_deadline").await else {
        return;
    };
    let policy = f.policy(&f.b);
    let raw = f.capture(&policy, 0).await.unwrap();
    let sampled = Arc::new(AtomicBool::new(false));
    let clock = sampled.clone();
    let b = eligible_b2a(f.b.boot().await.unwrap().with_clock(Arc::new(move || {
        clock.store(true, Ordering::SeqCst);
        110
    })));
    let end = Instant::now() + std::time::Duration::from_millis(30);
    let result = b
        .apply_inner(
            peer(&policy.origin),
            &raw,
            &policy,
            end,
            None,
            |bytes, policy| {
                let verified = snapshot::verify_snapshot(bytes, policy)?;
                while Instant::now() < end {
                    std::thread::sleep(end.saturating_duration_since(Instant::now()));
                }
                Ok(verified)
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(SnapshotEffectError::Effect(EffectError::Timeout))
    ));
    assert!(!sampled.load(Ordering::SeqCst));
    assert_eq!(effects(&f.b).await, (0, 0, 0, 0));
    let origin = peer(&policy.origin);
    b.0.suspend_history(origin, 0, deadline()).await.unwrap();
    assert!(matches!(
        b.apply_b2a(origin, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::Effect(EffectError::SuspendedHistory {
            blocked_next_seq: 1
        }))
    ));
    let blocked: i64 =
        sqlx::query_scalar("SELECT blocked_next_seq FROM federation_replica_resume_block")
            .fetch_one(&f.b.pool)
            .await
            .unwrap();
    assert_eq!(blocked, 1);
    assert_eq!(effects(&f.b).await, (0, 0, 0, 0));
}

#[tokio::test]
async fn origem_fixa_corte_antes_de_publicacao_posterior() {
    use crate::replication::NodeContext;
    let Some(f) = Three::new("snapshot_fixed_cut").await else {
        return;
    };
    let policy = f.policy(&f.b);
    let mut gate = f.a.pool.begin().await.unwrap();
    let gate_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    sqlx::query("LOCK TABLE federation_replica_proof IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *gate)
        .await
        .unwrap();
    let capturing = f.capture(&policy, 0);
    tokio::pin!(capturing);
    let reader = tokio::select! {
        result = &mut capturing => panic!("captura não aguardou a prova: {result:?}"),
        pid = wait_on(&f.a.pool,gate_pid) => pid,
    };
    let later = [f.a.frame(4)];
    let writing = async {
        let mut tx = f.a.pool.begin().await.unwrap();
        let mut guard = f.source.portable.lock_cluster_guard(&mut tx).await.unwrap();
        let key = f.a.origin.public_key_hex();
        fixture::admit_frames(
            &mut guard,
            &f.a.config.namespace,
            NodeContext {
                node_id: f.a.origin.node_id(),
                public_key: &key,
            },
            &later,
        )
        .await;
        tx.commit().await.unwrap();
    };
    tokio::pin!(writing);
    tokio::select! {
        () = &mut writing => panic!("publicação não aguardou o guard da captura"),
        _ = wait_on(&f.a.pool,reader) => {}
    }
    gate.commit().await.unwrap();
    let raw = capturing.await.unwrap();
    writing.await;
    let verified = snapshot::verify_snapshot(&raw, &policy).unwrap();
    assert_eq!(verified.cut(), (0, 3));
    assert_eq!(verified.events(), f.frames);
    assert_eq!(effects(&f.a).await, (4, 4, 4, 0));
}

fn eligible_b2a(service: ReplicaService) -> EligibleReceiver {
    EligibleReceiver(Arc::new(service), std::num::NonZeroU64::MIN)
}

async fn witness(f: &ReceiverFixture) -> HarnessHistory {
    HarnessHistory {
        database: sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        generation: std::num::NonZeroU64::new(1).unwrap(),
    }
}

async fn eligible(f: &ReceiverFixture, service: ReplicaService) -> EligibleReceiver {
    EligibleReceiver::new(Arc::new(service), witness(f).await, deadline())
        .await
        .unwrap()
}

async fn durable_attempt(f: &ReceiverFixture) -> (i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM federation_snapshot_intent_marker),
        (SELECT count(*) FROM federation_snapshot_intent_detail)",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn intencao_reconciliacao_e_fence_sobrevivem_reinicio_com_a2_bloqueada() {
    let Some(f) = Three::new("intent_restart").await else {
        return;
    };
    let policy = f.policy(&f.b);
    let remote = peer(&policy.origin);
    let raw = f.capture(&policy, 0).await.unwrap();
    let mut wrong = witness(&f.b).await;
    wrong.database.push_str("_unknown");
    assert!(matches!(
        EligibleReceiver::new(Arc::new(f.b.boot().await.unwrap()), wrong, deadline()).await,
        Err(SnapshotEffectError::Eligibility)
    ));
    let b = eligible(&f.b, f.b.boot().await.unwrap()).await;
    assert_eq!(b.generation().get(), 1);
    assert_eq!(b.checkpoint(remote, deadline()).await.unwrap(), 0);
    let first = b.intend(remote, &raw, &policy, deadline()).await.unwrap();
    assert_eq!(durable_attempt(&f.b).await, (1, 1));
    assert!(matches!(
        b.intend(remote, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::IntentPending)
    ));
    drop(b);
    let a2 = f.b.boot().await.unwrap();
    assert!(matches!(
        a2.checkpoint(remote, deadline()).await,
        Err(EffectError::SnapshotPending)
    ));
    assert!(matches!(
        a2.apply_copy(
            remote,
            Limits::new(8, 4096, 16384).unwrap(),
            &f.frames,
            deadline()
        )
        .await,
        Err(EffectError::SnapshotPending)
    ));
    let b = eligible(&f.b, a2).await;
    assert_eq!(
        b.recover(remote, &policy, deadline()).await.unwrap(),
        Recovery::NoEffect { after_seq: 0 }
    );
    assert!(matches!(
        b.apply(remote, first, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::AttemptMismatch)
    ));
    let second = b.intend(remote, &raw, &policy, deadline()).await.unwrap();
    assert!(second.get() > first.get());
    b.apply(remote, second, &raw, &policy, deadline())
        .await
        .unwrap();
    assert_eq!(durable_attempt(&f.b).await, (1, 1));
    assert_eq!(effects(&f.b).await, (3, 3, 3, 1));
    assert!(matches!(
        b.apply(remote, second, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::CheckpointMismatch)
    ));
    drop(b);

    let c_policy = f.policy(&f.c);
    let c_remote = peer(&c_policy.origin);
    let c = eligible(&f.c, f.c.boot().await.unwrap()).await;
    c.0.apply_copy(
        c_remote,
        Limits::new(8, 4096, 16384).unwrap(),
        &f.frames[..1],
        deadline(),
    )
    .await
    .unwrap();
    let c_raw = f.capture(&c_policy, 1).await.unwrap();
    let token = c
        .intend(c_remote, &c_raw, &c_policy, deadline())
        .await
        .unwrap();
    c.apply(c_remote, token, &c_raw, &c_policy, deadline())
        .await
        .unwrap();
    assert_eq!(
        c.recover(c_remote, &c_policy, deadline()).await.unwrap(),
        Recovery::Applied { after_seq: 3 }
    );

    let b = eligible(&f.b, f.b.boot().await.unwrap()).await;
    assert!(matches!(
        b.checkpoint(remote, deadline()).await,
        Err(SnapshotEffectError::Effect(EffectError::SnapshotPending))
    ));
    assert_eq!(
        b.recover(remote, &policy, deadline()).await.unwrap(),
        Recovery::Applied { after_seq: 3 }
    );
    assert_eq!(b.checkpoint(remote, deadline()).await.unwrap(), 3);
    assert_eq!(durable_attempt(&f.b).await, (0, 0));
    assert_eq!(effects(&f.a).await, (3, 3, 3, 0));
}

#[tokio::test]
async fn ttl_detalhe_reduzido_revogacao_e_falha_leitura_bloqueiam_sem_escrita() {
    use std::sync::atomic::{AtomicI64, Ordering};
    let Some(f) = Three::new("intent_ttl").await else {
        return;
    };
    let policy = f.policy(&f.b);
    let remote = peer(&policy.origin);
    let raw = f.capture(&policy, 0).await.unwrap();
    let clock = Arc::new(AtomicI64::new(110));
    let b = eligible(
        &f.b,
        f.b.boot().await.unwrap().with_clock({
            let clock = clock.clone();
            Arc::new(move || clock.load(Ordering::SeqCst))
        }),
    )
    .await;
    let token = b.intend(remote, &raw, &policy, deadline()).await.unwrap();
    let delete = sqlx::query("DELETE FROM federation_snapshot_intent_policy")
        .execute(&f.b.pool)
        .await
        .unwrap_err();
    assert_eq!(
        delete.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    sqlx::query("UPDATE federation_snapshot_intent_policy SET active=false")
        .execute(&f.b.pool)
        .await
        .unwrap();
    assert!(matches!(
        b.apply(remote, token, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::IntentPolicy)
    ));
    sqlx::query("INSERT INTO federation_snapshot_intent_policy
        SELECT policy_id,2,local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,
           jurisdiction,purpose,500,true,true FROM federation_snapshot_intent_policy WHERE policy_version=1")
        .execute(&f.b.pool).await.unwrap();
    clock.store(120, Ordering::SeqCst);
    assert!(matches!(
        b.apply(remote, token, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::IntentExpired)
    ));
    assert_eq!(effects(&f.b).await, (0, 0, 0, 0));
    assert_eq!(durable_attempt(&f.b).await, (1, 1));
    b.reduce_expired(remote, token, deadline()).await.unwrap();
    assert_eq!(durable_attempt(&f.b).await, (1, 0));
    assert!(matches!(
        b.apply(remote, token, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::IntentInconsistent)
    ));
    assert_eq!(
        b.recover(remote, &policy, deadline()).await.unwrap(),
        Recovery::Blocked { attempt: token }
    );
    drop(b);

    let a2 = f.b.boot().await.unwrap();
    assert!(matches!(
        a2.checkpoint(remote, deadline()).await,
        Err(EffectError::SnapshotPending)
    ));
    sqlx::query("ALTER TABLE federation_snapshot_intent_marker RENAME TO broken_intent_marker")
        .execute(&f.b.pool)
        .await
        .unwrap();
    assert!(matches!(
        a2.checkpoint(remote, deadline()).await,
        Err(EffectError::Storage(_))
    ));
    assert!(matches!(
        a2.apply_copy(
            remote,
            Limits::new(8, 4096, 16384).unwrap(),
            &f.frames,
            deadline()
        )
        .await,
        Err(EffectError::Storage(_))
    ));
    assert_eq!(effects(&f.b).await, (0, 0, 0, 0));
}

#[tokio::test]
async fn corrida_apply_recover_e_revogacao_preserva_intencao() {
    let Some(f) = Three::new("intent_order").await else {
        return;
    };
    let policy = f.policy(&f.b);
    let remote = peer(&policy.origin);
    let raw = f.capture(&policy, 0).await.unwrap();
    let b = eligible(&f.b, f.b.boot().await.unwrap()).await;
    let token = b.intend(remote, &raw, &policy, deadline()).await.unwrap();

    let mut blocker = f.b.pool.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("LOCK TABLE federation_snapshot_receipt IN SHARE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let timeout = b
        .apply(
            remote,
            token,
            &raw,
            &policy,
            Instant::now() + std::time::Duration::from_millis(120),
        )
        .await;
    assert!(
        matches!(
            timeout,
            Err(SnapshotEffectError::Effect(EffectError::Timeout))
        ),
        "{timeout:?}"
    );
    assert_eq!(effects(&f.b).await, (0, 0, 0, 0));
    assert_eq!(durable_attempt(&f.b).await, (1, 1));

    let work = b.apply(remote, token, &raw, &policy, deadline());
    tokio::pin!(work);
    let writer = tokio::select! {
        outcome = &mut work => panic!("apply não aguardou a barreira: {outcome:?}"),
        pid = wait_on(&f.b.pool,blocker_pid) => pid,
    };
    let reconcile = b.recover(remote, &policy, deadline());
    tokio::pin!(reconcile);
    tokio::select! {
        outcome = &mut reconcile => panic!("recover passou à frente: {outcome:?}"),
        _ = wait_on(&f.b.pool,writer) => {}
    }
    blocker.commit().await.unwrap();
    work.await.unwrap();
    assert_eq!(reconcile.await.unwrap(), Recovery::Applied { after_seq: 3 });
    assert_eq!(durable_attempt(&f.b).await, (0, 0));

    let policy_c = f.policy(&f.c);
    let remote_c = peer(&policy_c.origin);
    let raw_c = f.capture(&policy_c, 0).await.unwrap();
    let c = eligible(&f.c, f.c.boot().await.unwrap()).await;
    let c_token = c
        .intend(remote_c, &raw_c, &policy_c, deadline())
        .await
        .unwrap();
    let mut gate = f.c.pool.begin().await.unwrap();
    let gate_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    sqlx::query("LOCK TABLE federation_snapshot_intent_marker IN SHARE MODE")
        .execute(&mut *gate)
        .await
        .unwrap();
    let resolving_first = c.recover(remote_c, &policy_c, deadline());
    tokio::pin!(resolving_first);
    let resolver = tokio::select! {
        outcome = &mut resolving_first => panic!("recover não aguardou DELETE: {outcome:?}"),
        pid = wait_on(&f.c.pool,gate_pid) => pid,
    };
    let old_executor = c.apply(remote_c, c_token, &raw_c, &policy_c, deadline());
    tokio::pin!(old_executor);
    tokio::select! {
        outcome = &mut old_executor => panic!("executor passou à frente: {outcome:?}"),
        _ = wait_on(&f.c.pool,resolver) => {}
    }
    gate.commit().await.unwrap();
    assert_eq!(
        resolving_first.await.unwrap(),
        Recovery::NoEffect { after_seq: 0 }
    );
    assert!(matches!(
        old_executor.await,
        Err(SnapshotEffectError::AttemptMismatch)
    ));
    assert_eq!(effects(&f.c).await, (0, 0, 0, 0));
    let c_token = c
        .intend(remote_c, &raw_c, &policy_c, deadline())
        .await
        .unwrap();
    let mut revoke = f.c.pool.begin().await.unwrap();
    let revoke_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *revoke)
        .await
        .unwrap();
    sqlx::query("UPDATE federation_replica_acl SET revoked=true WHERE direction='import'")
        .execute(&mut *revoke)
        .await
        .unwrap();
    let reconciling = c.recover(remote_c, &policy_c, deadline());
    tokio::pin!(reconciling);
    tokio::select! {
        outcome = &mut reconciling => panic!("recover não aguardou revogação: {outcome:?}"),
        _ = wait_on(&f.c.pool,revoke_pid) => {}
    }
    revoke.commit().await.unwrap();
    assert!(matches!(
        reconciling.await,
        Err(SnapshotEffectError::Effect(EffectError::Replication(
            ReplicaError::AclDenied
        )))
    ));
    assert!(matches!(
        c.apply(remote_c, c_token, &raw_c, &policy_c, deadline())
            .await,
        Err(SnapshotEffectError::Effect(EffectError::Replication(
            ReplicaError::AclDenied
        )))
    ));
    assert_eq!(effects(&f.c).await, (0, 0, 0, 0));
    assert_eq!(durable_attempt(&f.c).await, (1, 1));
}

#[tokio::test]
async fn recover_recusa_digest_divergente_ou_recibo_expirado() {
    use std::sync::atomic::{AtomicI64, Ordering};
    let Some(f) = Three::new("intent_divergence").await else {
        return;
    };
    let policy = f.policy(&f.b);
    let remote = peer(&policy.origin);
    let raw = f.capture(&policy, 0).await.unwrap();
    let clock = Arc::new(AtomicI64::new(110));
    let b = eligible(
        &f.b,
        f.b.boot().await.unwrap().with_clock({
            let clock = clock.clone();
            Arc::new(move || clock.load(Ordering::SeqCst))
        }),
    )
    .await;
    let token = b.intend(remote, &raw, &policy, deadline()).await.unwrap();
    b.apply(remote, token, &raw, &policy, deadline())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE federation_replica_proof SET frame=decode('20','hex') || frame WHERE seq=2",
    )
    .execute(&f.b.pool)
    .await
    .unwrap();
    let blocked = Recovery::Blocked { attempt: token };
    assert_eq!(
        b.recover(remote, &policy, deadline()).await.unwrap(),
        blocked
    );
    sqlx::query("UPDATE federation_replica_proof SET frame=$1 WHERE seq=2")
        .bind(&f.frames[1])
        .execute(&f.b.pool)
        .await
        .unwrap();
    clock.store(130, Ordering::SeqCst);
    assert_eq!(
        b.recover(remote, &policy, deadline()).await.unwrap(),
        blocked
    );
    assert_eq!(durable_attempt(&f.b).await, (1, 1));
    assert_eq!(effects(&f.b).await, (3, 3, 3, 1));
}

#[tokio::test]
async fn falha_no_detalhe_desfaz_marcador_e_suspensao_recusa_intencao() {
    let Some(f) = Three::new("intent_atomic").await else {
        return;
    };
    let policy = f.policy(&f.b);
    let remote = peer(&policy.origin);
    let raw = f.capture(&policy, 0).await.unwrap();
    let b = eligible(&f.b, f.b.boot().await.unwrap()).await;
    sqlx::query(
        "ALTER TABLE federation_snapshot_intent_detail
        ADD CONSTRAINT fail_detail CHECK (cut_seq <> 3)",
    )
    .execute(&f.b.pool)
    .await
    .unwrap();
    assert!(matches!(
        b.intend(remote, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::Effect(EffectError::Storage(_)))
    ));
    assert_eq!(durable_attempt(&f.b).await, (0, 0));
    assert_eq!(effects(&f.b).await, (0, 0, 0, 0));
    assert_eq!(b.checkpoint(remote, deadline()).await.unwrap(), 0);
    sqlx::query("ALTER TABLE federation_snapshot_intent_detail DROP CONSTRAINT fail_detail")
        .execute(&f.b.pool)
        .await
        .unwrap();
    b.0.suspend_history(remote, 0, deadline()).await.unwrap();
    assert!(matches!(
        b.intend(remote, &raw, &policy, deadline()).await,
        Err(SnapshotEffectError::Effect(EffectError::SuspendedHistory {
            blocked_next_seq: 1
        }))
    ));
    assert_eq!(durable_attempt(&f.b).await, (0, 0));
}
