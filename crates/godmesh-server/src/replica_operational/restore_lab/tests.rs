#[test]
fn estados_distinguem_cas_de_abertura() {
    use super::LocalPhase;
    assert_ne!(LocalPhase::ReconciledClosed, LocalPhase::Open);
    assert_ne!(LocalPhase::RetiredClosed, LocalPhase::Open);
}

use super::super::fixture;
use fixture::{CLASS, CLUSTER, ReceiverFixture, boot_source, deadline, isolated_database};

use godmesh_federation::{
    NodeIdentity,
    restore_evidence::{
        BackupKind, ExpectedHighWater, ExpectedPromotion, Fence, IntentState, RestoreHighWaterV1,
        RestorePromotionV1, VerifiedHighWater, VerifiedPromotion, verify_high_water,
        verify_promotion,
    },
    snapshot::{SnapshotIdentity, SnapshotLimits, SnapshotPolicy, verify_snapshot},
};
use sha2::{Digest, Sha256};
use std::{num::NonZeroUsize, sync::Arc};

use super::*;
use crate::{
    replica_operational::{ReceiverState, snapshot_harness::Window},
    replication::Limits,
};

fn identity(id: &NodeIdentity) -> SnapshotIdentity {
    SnapshotIdentity {
        node_id: id.node_id().into(),
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

fn signed_high(mut body: RestoreHighWaterV1, signer: &NodeIdentity) -> VerifiedHighWater {
    body.sign(signer).unwrap();
    verify_high_water(
        &serde_json::to_vec(&body).unwrap(),
        NonZeroUsize::new(16_384).unwrap(),
        &ExpectedHighWater {
            key: &body.key,
            target_db_id: &body.target_db_id,
            fenced_generation: body.fenced_generation,
            target_generation: body.target_generation,
            high_water_sha256: body.digest().unwrap(),
            fence_sha256: body.fence_sha256,
            harness_public_key: &signer.public_key_hex(),
        },
    )
    .unwrap()
}

fn signed_promotion(
    high: &VerifiedHighWater,
    reconciliation: &godmesh_federation::restore_evidence::Reconciliation,
    signer: &NodeIdentity,
) -> VerifiedPromotion {
    let mark = high.high_water();
    let mut body = RestorePromotionV1 {
        version: 1,
        key: mark.key.clone(),
        target_db_id: mark.target_db_id.clone(),
        fenced_generation: mark.fenced_generation,
        target_generation: mark.target_generation,
        high_water_sha256: high.digest(),
        reconciliation_sha256: reconciliation.digest().unwrap(),
        fence_sha256: mark.fence_sha256,
        not_before: 100,
        expires_at: 200,
        signature: [0; 64],
    };
    body.sign(signer).unwrap();
    verify_promotion(
        &serde_json::to_vec(&body).unwrap(),
        NonZeroUsize::new(16_384).unwrap(),
        &ExpectedPromotion {
            high_water: high,
            reconciliation,
        },
        110,
    )
    .unwrap()
}

async fn local(f: &ReceiverFixture, binding: Binding) -> (LocalRestore, Arc<ReplicaService>) {
    let gate = AdmissionGate::new(binding).unwrap();
    let svc = Arc::new(f.boot().await.unwrap().with_restore_gate(gate.clone()));
    (LocalRestore::new(svc.clone(), gate).unwrap(), svc)
}

#[tokio::test]
async fn restore_corte_fixo_cas_fechado_reinicio_e_cerca_irreversivel() {
    let Some(a) = ReceiverFixture::new("restore_a").await else {
        return;
    };
    let mut cut = ReceiverFixture::new_with(
        "restore_cut",
        a.origin.clone(),
        a.receiver.clone(),
        &a.config.namespace.tenant_id,
    )
    .await
    .unwrap();
    cut.config.max_events = 1.try_into().unwrap();
    let b = ReceiverFixture::new_with(
        "restore_target",
        a.origin.clone(),
        a.receiver.clone(),
        &a.config.namespace.tenant_id,
    )
    .await
    .unwrap();
    let external = isolated_database("restore_external").await.unwrap();
    for f in [&cut, &b] {
        f.reserve().await;
        sqlx::raw_sql(LOCAL_DDL).execute(&f.pool).await.unwrap();
        sqlx::query(
            "INSERT INTO federation_snapshot_receipt_policy
            VALUES ('restore',1,$1,$2,$3,$4,0,7,'BR','snapshot_receipt',60,true)",
        )
        .bind(f.receiver.node_id())
        .bind(f.origin.node_id())
        .bind(&f.config.namespace.tenant_id)
        .bind(&f.config.namespace.stream_id)
        .execute(&f.pool)
        .await
        .unwrap();
    }
    sqlx::raw_sql(EXTERNAL_DDL)
        .execute(&external)
        .await
        .unwrap();
    let clock_time = Arc::new(std::sync::atomic::AtomicI64::new(110));
    let clock = {
        let time = clock_time.clone();
        Arc::new(move || time.load(std::sync::atomic::Ordering::SeqCst))
    };
    let ext = ExternalStore::new(external.clone()).with_clock(clock.clone());
    let policy = SnapshotPolicy {
        cluster: CLUSTER.into(),
        origin: identity(&a.origin),
        destination: identity(&b.receiver),
        tenant_id: a.config.namespace.tenant_id.clone(),
        stream_id: a.config.namespace.stream_id.clone(),
        partition: 0,
        authority_epoch: 7,
        retention_class: CLASS.into(),
        limits: SnapshotLimits {
            max_package_bytes: 16_384.try_into().unwrap(),
            max_manifest_bytes: 4096.try_into().unwrap(),
            max_event_bytes: 4096.try_into().unwrap(),
            max_events: 12.try_into().unwrap(),
        },
    };
    let key = RestoreKey {
        fixture_id: "22".repeat(16),
        cluster: policy.cluster.clone(),
        local_node: policy.destination.node_id.clone(),
        origin_node: policy.origin.node_id.clone(),
        tenant_id: policy.tenant_id.clone(),
        stream_id: policy.stream_id.clone(),
        partition: 0,
        authority_epoch: 7,
    };
    let frames: Vec<_> = (1..=3)
        .map(|seq| [b" \t".to_vec(), a.frame(seq), b"\n".to_vec()].concat())
        .collect();
    let source = boot_source(
        &a,
        Arc::new(NodeIdentity::generate_in_memory().unwrap()),
        &frames,
    )
    .await
    .with_clock(clock.clone());
    let a_peer = peer(&policy.origin);
    let b_peer = peer(&policy.destination);
    let first = Binding {
        key: key.clone(),
        generation: 1,
        target_db_id: allocation_id(&cut.pool).await.unwrap(),
        high_water_sha256: [11; 32],
        fence_sha256: [12; 32],
    };
    ext.seed(&first, deadline()).await.unwrap();
    let seeded = ext.observe(&key, deadline()).await.unwrap();
    let (old, old_service) = local(&cut, first.clone()).await;
    old.boot_initial(&seeded, deadline()).await.unwrap();
    for frame in &frames[..2] {
        old_service
            .apply_copy(
                a_peer,
                Limits::new(1, 4096, 16_384).unwrap(),
                std::slice::from_ref(frame),
                deadline(),
            )
            .await
            .unwrap();
    }
    old.retire(deadline()).await.unwrap();
    assert!(matches!(
        old.boot_initial(&seeded, deadline()).await,
        Err(Error::Retired)
    ));
    assert!(matches!(
        old_service.checkpoint(a_peer, deadline()).await,
        Err(EffectError::RestoreClosed)
    ));
    let a_observed = observe_origin(&source, &key, b_peer, deadline())
        .await
        .unwrap();
    let fixed = observe_cut(&old_service, &key, 1, a_peer, &a_observed, 0, deadline())
        .await
        .unwrap();
    assert_eq!(fixed.control.prefix, 2);
    let fence = Fence {
        key: key.clone(),
        fenced_generation: 1,
        cut_db_id: first.target_db_id.clone(),
        target_db_id: allocation_id(&b.pool).await.unwrap(),
        old_login: "fixture_old".into(),
        admission_closed: true,
        nologin_confirmed: true,
        grants_removed: true,
        remaining_sessions: 0,
        unknown_effects: 0,
        fenced_at: 109,
    };
    let artifact = BackupArtifact {
        bytes: 7,
        sha256: Sha256::digest(b"fixture").into(),
    };
    let high_body = RestoreHighWaterV1 {
        version: 1,
        key: key.clone(),
        backup_db_id: first.target_db_id.clone(),
        target_db_id: fence.target_db_id.clone(),
        backup_generation: 1,
        fenced_generation: 1,
        target_generation: 2,
        backup_kind: BackupKind::BeforeCut,
        backup_bytes: artifact.bytes,
        backup_sha256: artifact.sha256,
        backup_prefix: 0,
        cut_prefix: 2,
        backup_proof_count: 0,
        backup_proofs_sha256: fixed.backup_proofs_sha256,
        gap_proof_count: 2,
        gap_proofs_sha256: fixed.gap_proofs_sha256,
        control_state_sha256: fixed.control.digest().unwrap(),
        blocked_next_seq: 0,
        intent_state: IntentState::Absent,
        attempt_id: 0,
        fence_sha256: fence.digest().unwrap(),
        cut_at: fixed.cut_at,
        signature: [0; 64],
    };
    let signer = NodeIdentity::generate_in_memory().unwrap();
    let mut suspended = high_body.clone();
    suspended.blocked_next_seq = 3;
    let suspended = signed_high(suspended, &signer);
    assert!(matches!(
        capture_gap(
            &source,
            &a.origin,
            b_peer,
            &suspended,
            Window {
                not_before: 100,
                expires_at: 190
            },
            &policy,
            deadline()
        )
        .await,
        Err(Error::Incomplete)
    ));
    let mut pending = high_body.clone();
    pending.intent_state = IntentState::Pending;
    pending.attempt_id = 9;
    let pending = signed_high(pending, &signer);
    assert!(matches!(
        capture_gap(
            &source,
            &a.origin,
            b_peer,
            &pending,
            Window {
                not_before: 100,
                expires_at: 190
            },
            &policy,
            deadline()
        )
        .await,
        Err(Error::Incomplete)
    ));
    let high = signed_high(high_body.clone(), &signer);
    let binding = Binding::from_high_water(&high).unwrap();
    assert!(ext.publish(&high, &fence, deadline()).await.is_err());
    assert_eq!(
        ext.observe(&key, deadline()).await.unwrap().binding(),
        &first
    );
    let prior_logins = GenerationLogins {
        target_login: "fixture_old".into(),
        observer_login: "fixture_observer".into(),
        cas_login: "fixture_cas".into(),
    };
    let target_logins = GenerationLogins {
        target_login: "fixture_new".into(),
        ..prior_logins.clone()
    };
    ext.register_logins(&first, &prior_logins, deadline())
        .await
        .unwrap();
    ext.register_logins(&first, &prior_logins, deadline())
        .await
        .unwrap();
    assert_eq!(ext.logins(&first, deadline()).await.unwrap(), prior_logins);
    assert!(matches!(
        ext.register_logins(&first, &target_logins, deadline())
            .await,
        Err(Error::Conflict)
    ));
    let mut wrong_database = first.clone();
    wrong_database.target_db_id = binding.target_db_id.clone();
    assert!(matches!(
        ext.logins(&wrong_database, deadline()).await,
        Err(Error::Conflict)
    ));
    assert!(matches!(
        ext.register_logins(&wrong_database, &prior_logins, deadline())
            .await,
        Err(Error::Conflict)
    ));
    let mut other_key = binding.clone();
    other_key.key.stream_id.push_str("-other");
    for another in [&binding, &other_key] {
        assert!(matches!(
            ext.register_logins(another, &prior_logins, deadline())
                .await,
            Err(Error::Conflict)
        ));
    }
    let mut duplicate_role = target_logins.clone();
    duplicate_role.cas_login = duplicate_role.observer_login.clone();
    assert!(matches!(
        ext.register_logins(&binding, &duplicate_role, deadline())
            .await,
        Err(Error::Scope)
    ));
    assert!(matches!(
        ext.publish(&high, &fence, deadline()).await,
        Err(Error::Incomplete)
    ));
    ext.register_logins(&binding, &target_logins, deadline())
        .await
        .unwrap();
    assert_eq!(
        ext.logins(&binding, deadline()).await.unwrap(),
        target_logins
    );
    for mutation in [
        "UPDATE federation_restore_generation_logins SET target_login='replacement'",
        "DELETE FROM federation_restore_generation_logins",
        "TRUNCATE federation_restore_generation_logins",
    ] {
        let error = sqlx::query(mutation).execute(&external).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }

    let mut wrong_fence = fence.clone();
    wrong_fence.old_login = target_logins.target_login;
    let mut wrong_high = high_body;
    wrong_high.fence_sha256 = wrong_fence.digest().unwrap();
    let wrong_high = signed_high(wrong_high, &signer);
    assert!(matches!(
        ext.publish(&wrong_high, &wrong_fence, deadline()).await,
        Err(Error::Conflict)
    ));
    assert_eq!(
        ext.observe(&key, deadline()).await.unwrap().binding(),
        &first
    );
    ext.publish(&high, &fence, deadline()).await.unwrap();
    ext.publish(&high, &fence, deadline()).await.unwrap();
    let observed = ext
        .record_origin(&binding, &a_observed, deadline())
        .await
        .unwrap();
    let (local_b, gated) = local(&b, binding.clone()).await;
    assert_eq!(
        local_b.boot_closed(&observed, deadline()).await.unwrap(),
        LocalPhase::RestoreClosed
    );
    assert!(matches!(
        gated.checkpoint(a_peer, deadline()).await,
        Err(EffectError::RestoreClosed)
    ));
    let package = capture_gap(
        &source,
        &a.origin,
        b_peer,
        &high,
        Window {
            not_before: 100,
            expires_at: 190,
        },
        &policy,
        deadline(),
    )
    .await
    .unwrap();
    assert_eq!(verify_snapshot(&package, &policy).unwrap().cut(), (0, 2));
    assert_eq!(
        verify_snapshot(&package, &policy).unwrap().events(),
        &frames[..2]
    );
    sqlx::query(
        "INSERT INTO federation_replica_resume_block
        (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch,blocked_next_seq)
        VALUES ($1,$2,$3,$4,0,7,1)",
    )
    .bind(&key.local_node)
    .bind(&key.origin_node)
    .bind(&key.tenant_id)
    .bind(&key.stream_id)
    .execute(&b.pool)
    .await
    .unwrap();
    assert!(
        local_b
            .reconcile(
                a_peer,
                &high,
                &observed,
                &a_observed,
                artifact,
                Some((&package, &policy)),
                deadline()
            )
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT blocked_next_seq FROM federation_replica_resume_block"
        )
        .fetch_one(&b.pool)
        .await
        .unwrap(),
        1
    );
    sqlx::query("DELETE FROM federation_replica_resume_block")
        .execute(&b.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO federation_snapshot_intent_marker
        (local_node,origin_node,tenant_id,stream_id,partition,authority_epoch) VALUES ($1,$2,$3,$4,0,7)")
        .bind(&key.local_node).bind(&key.origin_node).bind(&key.tenant_id).bind(&key.stream_id)
        .execute(&b.pool).await.unwrap();
    assert!(
        local_b
            .reconcile(
                a_peer,
                &high,
                &observed,
                &a_observed,
                artifact,
                Some((&package, &policy)),
                deadline()
            )
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM federation_snapshot_intent_marker")
            .fetch_one(&b.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT prefix FROM federation_replica_checkpoint")
            .fetch_one(&b.pool)
            .await
            .unwrap(),
        0
    );
    sqlx::query("DELETE FROM federation_snapshot_intent_marker")
        .execute(&b.pool)
        .await
        .unwrap();
    let mut damaged = package.clone();
    *damaged.last_mut().unwrap() ^= 1;
    assert!(
        local_b
            .reconcile(
                a_peer,
                &high,
                &observed,
                &a_observed,
                artifact,
                Some((&damaged, &policy)),
                deadline()
            )
            .await
            .is_err()
    );
    assert_eq!(
        local_b.phase(deadline()).await.unwrap(),
        LocalPhase::RestoreClosed
    );
    let result = local_b
        .reconcile(
            a_peer,
            &high,
            &observed,
            &a_observed,
            artifact,
            Some((&package, &policy)),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(result.final_prefix, 2);
    assert_eq!(
        local_b.phase(deadline()).await.unwrap(),
        LocalPhase::ReconciledClosed
    );
    sqlx::query("UPDATE federation_restore_local SET final_prefix=3")
        .execute(&b.pool)
        .await
        .unwrap();
    assert!(matches!(
        local_b
            .promotion_candidate(a_peer, &high, &a_observed, deadline())
            .await,
        Err(Error::Conflict)
    ));
    sqlx::query("UPDATE federation_restore_local SET final_prefix=2")
        .execute(&b.pool)
        .await
        .unwrap();
    let promotion = signed_promotion(&high, &result, &signer);
    assert!(matches!(
        local_b
            .consume_promotion(a_peer, &high, &ext, &promotion, &a_observed, deadline())
            .await,
        Err(Error::Conflict)
    ));
    let mut mismatched = a_observed.clone();
    mismatched.0.m3.version += 1;
    assert!(
        local_b
            .prepare_promotion(a_peer, &high, &promotion, &mismatched, deadline())
            .await
            .is_err()
    );
    local_b
        .prepare_promotion(a_peer, &high, &promotion, &a_observed, deadline())
        .await
        .unwrap();
    sqlx::query("UPDATE federation_replica_acl SET revoked=true")
        .execute(&b.pool)
        .await
        .unwrap();
    let refused = local_b
        .consume_promotion(a_peer, &high, &ext, &promotion, &a_observed, deadline())
        .await;
    assert!(
        refused.is_err(),
        "B com ACL revogada não pode consumir a autorização"
    );
    assert_eq!(
        ext.observe(&key, deadline()).await.unwrap().phase(),
        ExternalPhase::Fenced
    );
    sqlx::query("UPDATE federation_replica_acl SET revoked=false")
        .execute(&b.pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE FUNCTION hold_restore_commit() RETURNS trigger LANGUAGE plpgsql AS $$
           BEGIN PERFORM pg_advisory_xact_lock(73104); RETURN NEW; END $$;
         CREATE CONSTRAINT TRIGGER hold_restore_commit AFTER INSERT ON federation_restore_decision
           DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION hold_restore_commit()",
    )
    .execute(&external)
    .await
    .unwrap();
    let mut blocker = external.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(73104)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let commit_deadline = deadline();
    let consume = local_b.consume_promotion(
        a_peer,
        &high,
        &ext,
        &promotion,
        &a_observed,
        commit_deadline,
    );
    let barrier = async {
        tokio::time::timeout_at(commit_deadline, async {
            loop {
                let waiting: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM pg_stat_activity
                     WHERE datname=current_database() AND upper(query)='COMMIT'
                     AND $1 = ANY(pg_blocking_pids(pid)))",
                )
                .bind(blocker_pid)
                .fetch_one(&external)
                .await
                .unwrap();
                if waiting {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("CAS deve chegar ao COMMIT bloqueado");
        let mut change = b.pool.acquire().await.unwrap();
        let change_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *change)
            .await
            .unwrap();
        let update =
            sqlx::query("UPDATE federation_replica_acl SET revoked=true").execute(&mut *change);
        let wait_for_update = tokio::time::timeout_at(commit_deadline, async {
            loop {
                let waiting: bool =
                    sqlx::query_scalar("SELECT cardinality(pg_blocking_pids($1)) > 0")
                        .bind(change_pid)
                        .fetch_one(&b.pool)
                        .await
                        .unwrap();
                if waiting {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        });
        let (updated, waited) = tokio::join!(update, wait_for_update);
        waited.expect("alteração concorrente deve aguardar o guard de B");
        assert_eq!(updated.unwrap().rows_affected(), 1);
    };
    let (uncertain, ()) = tokio::join!(consume, barrier);
    assert!(matches!(
        uncertain,
        Err(Error::Effect(EffectError::CommitUnknown(_)))
    ));
    assert!(matches!(
        gated.checkpoint(a_peer, deadline()).await,
        Err(EffectError::RestoreClosed)
    ));
    blocker.rollback().await.unwrap();
    let resolved = ext.resolve_promotion(&key, deadline()).await.unwrap();
    match resolved.phase() {
        ExternalPhase::Fenced => assert_eq!(resolved.authorization_sha256(), None),
        ExternalPhase::PromotionConsumed => {
            assert_eq!(resolved.authorization_sha256(), Some(promotion.digest()));
        }
    }
    assert!(resolved.opening_origin().is_none());
    sqlx::query("UPDATE federation_replica_acl SET revoked=false")
        .execute(&b.pool)
        .await
        .unwrap();

    local_b
        .consume_promotion(a_peer, &high, &ext, &promotion, &a_observed, deadline())
        .await
        .unwrap();
    local_b
        .consume_promotion(a_peer, &high, &ext, &promotion, &a_observed, deadline())
        .await
        .unwrap();
    assert!(matches!(
        gated.checkpoint(a_peer, deadline()).await,
        Err(EffectError::RestoreClosed)
    ));
    drop(local_b);

    let observed = ext.observe(&key, deadline()).await.unwrap();
    assert!(observed.opening_origin().is_none());
    let gate = AdmissionGate::new(binding.clone()).unwrap();
    let gated = Arc::new(
        b.boot()
            .await
            .unwrap()
            .with_clock(clock.clone())
            .with_restore_gate(gate.clone()),
    );
    let local_b = LocalRestore::new(gated.clone(), gate).unwrap();
    local_b.boot_closed(&observed, deadline()).await.unwrap();
    assert!(
        local_b
            .open(
                a_peer,
                &high,
                &observed,
                &promotion,
                &a_observed,
                deadline()
            )
            .await
            .is_err()
    );
    assert!(
        local_b
            .opening_candidate(a_peer, &high, &observed, deadline())
            .await
            .is_err()
    );
    assert!(
        observe_opening_origin(&source, &observed, a_peer, deadline())
            .await
            .is_err()
    );
    assert!(
        ext.observe(&key, deadline())
            .await
            .unwrap()
            .opening_origin()
            .is_none()
    );

    let mut unavailable = a.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE federation_replica_acl IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *unavailable)
        .await
        .unwrap();
    let attempt = observe_opening_origin(
        &source,
        &observed,
        b_peer,
        tokio::time::Instant::now() + std::time::Duration::from_millis(250),
    )
    .await;
    assert!(attempt.is_err());
    let still_consumed = ext.observe(&key, deadline()).await.unwrap();
    assert_eq!(
        still_consumed.authorization_sha256(),
        observed.authorization_sha256()
    );
    assert_eq!(still_consumed.origin(), observed.origin());
    assert!(still_consumed.opening_origin().is_none());
    unavailable.rollback().await.unwrap();

    clock_time.store(111, std::sync::atomic::Ordering::SeqCst);
    let later = observe_opening_origin(&source, &observed, b_peer, deadline())
        .await
        .unwrap();
    // Um controle M3 diferente em B impede a abertura mesmo com evidência pós-CAS válida.
    let mut divergent = later.clone();
    divergent.observation.0.m3.version += 1;
    let different = ext
        .record_opening_origin(&divergent, deadline())
        .await
        .unwrap();
    assert!(
        local_b
            .open(
                a_peer,
                &high,
                &different,
                &promotion,
                different.opening_origin().unwrap(),
                deadline()
            )
            .await
            .is_err()
    );
    assert!(matches!(
        gated.checkpoint(a_peer, deadline()).await,
        Err(EffectError::RestoreClosed)
    ));
    assert_eq!(
        different.authorization_sha256(),
        observed.authorization_sha256()
    );

    let observed = ext.record_opening_origin(&later, deadline()).await.unwrap();
    assert_eq!(observed.origin(), Some(&a_observed));
    assert_ne!(observed.opening_origin(), observed.origin());
    local_b
        .opening_candidate(a_peer, &high, &observed, deadline())
        .await
        .unwrap();
    local_b
        .open(
            a_peer,
            &high,
            &observed,
            &promotion,
            observed.opening_origin().unwrap(),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        gated.checkpoint(a_peer, deadline()).await.unwrap(),
        ReceiverState::Ready { after_seq: 2 }
    );
    gated
        .apply_copy(
            a_peer,
            Limits::new(12, 4096, 16_384).unwrap(),
            &frames[2..],
            deadline(),
        )
        .await
        .unwrap();
    drop(local_b);
    clock_time.store(112, std::sync::atomic::Ordering::SeqCst);
    let current = ext.observe(&key, deadline()).await.unwrap();
    let origin = observe_opening_origin(&source, &current, b_peer, deadline())
        .await
        .unwrap();
    let observed = ext
        .record_opening_origin(&origin, deadline())
        .await
        .unwrap();
    let gate = AdmissionGate::new(binding).unwrap();
    let gated = Arc::new(
        b.boot()
            .await
            .unwrap()
            .with_clock(clock.clone())
            .with_restore_gate(gate.clone()),
    );
    let local_b = LocalRestore::new(gated.clone(), gate).unwrap();
    assert!(matches!(
        gated.checkpoint(a_peer, deadline()).await,
        Err(EffectError::RestoreClosed)
    ));
    assert_eq!(
        local_b.boot_closed(&observed, deadline()).await.unwrap(),
        LocalPhase::ReconciledClosed
    );
    local_b
        .opening_candidate(a_peer, &high, &observed, deadline())
        .await
        .unwrap();
    local_b
        .open(
            a_peer,
            &high,
            &observed,
            &promotion,
            observed.opening_origin().unwrap(),
            deadline(),
        )
        .await
        .unwrap();
    assert_eq!(
        gated.checkpoint(a_peer, deadline()).await.unwrap(),
        ReceiverState::Ready { after_seq: 3 }
    );
    local_b.retire(deadline()).await.unwrap();
    assert!(matches!(
        local_b
            .open(
                a_peer,
                &high,
                &observed,
                &promotion,
                observed.opening_origin().unwrap(),
                deadline()
            )
            .await,
        Err(Error::Retired)
    ));
    assert!(matches!(
        local_b.boot_closed(&observed, deadline()).await,
        Err(Error::Retired)
    ));
    let err = sqlx::query("UPDATE federation_restore_local SET phase='reconciled_closed'")
        .execute(&b.pool)
        .await
        .unwrap_err();
    assert_eq!(
        err.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    assert!(matches!(
        ext.seed(&first, deadline()).await,
        Err(Error::Conflict)
    ));
}
