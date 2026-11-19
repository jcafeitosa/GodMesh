mod common;

#[tokio::test]
async fn migra_reserva_aditiva_sem_semear_politica() {
    let Some(pool) = database("clean").await else {
        return;
    };
    let migrated: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM schema_migration WHERE nome = '0014_federation_replication.sql')",
    ).fetch_one(&pool).await.unwrap();
    assert!(migrated);
    let policies: i64 = sqlx::query_scalar("SELECT count(*) FROM federation_replica_policy")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(policies, 0);
}

#[test]
fn limites_explicitos_recusam_zero() {
    use godmesh_server::replication::Limits;
    assert!(Limits::new(0, 1024, 1024).is_err());
    assert!(Limits::new(1, 0, 1024).is_err());
    assert!(Limits::new(1, 1024, 0).is_err());
    assert!(Limits::new(1, 1024, 1024).is_ok());
}

use godmesh_federation::{
    NodeIdentity,
    event::{EventDraft, signed_event},
};
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::{
    auth::{Scope, Subject, VerifiedGrant},
    portable_join::{PortableJoin, PortableJoinConfig},
    replication::{
        CopyAuthority, Disposal, Error, Limits, Namespace, NodeContext, OriginAdmission, PolicyRef,
        Replication, Reservation, Reserved, Series,
    },
};
use sqlx::PgPool;
use std::sync::Arc;

const CLASS: &str = "m4a_metadados_posicao";

struct Fixture {
    pool: PgPool,
    portable: PortableJoin,
    a: Arc<NodeIdentity>,
    b: Arc<NodeIdentity>,
    c: Arc<NodeIdentity>,
    r: Reservation,
    grant: VerifiedGrant,
}

impl Fixture {
    async fn new() -> Option<Self> {
        let pool = common::pool().await?;
        let a = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let b = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        let c = Arc::new(NodeIdentity::generate_in_memory().unwrap());
        Some(Self::seed(pool, a, b, c).await)
    }

    async fn seed(
        pool: PgPool,
        a: Arc<NodeIdentity>,
        b: Arc<NodeIdentity>,
        c: Arc<NodeIdentity>,
    ) -> Self {
        let tenant = a.node_id().to_string();
        let stream = "m4a.metadata.sintetica";
        let pin = format!("{}:{}", a.node_id(), a.public_key_hex());
        let config = PortableJoinConfig::from_env_with(|key| {
            Some(
                match key {
                    "GODMESH_FEDERATION_GOVERNANCE" => "single_approver:m4a-test",
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
        let portable = PortableJoin::new(config, pool.clone(), a.clone());
        sqlx::query("INSERT INTO tenant_region (tenant_id,jurisdiction) VALUES ($1,'BR')")
            .bind(&tenant)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO retention_class VALUES ($1,'BR','fixture',1,'expiry') ON CONFLICT DO NOTHING")
            .bind(CLASS).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO federation_peer (node_id,peer_id,public_key,cluster,region,trust_state,software_version,epoch)
                     VALUES ($1,$1,$2,'m4a-test','lab','authorized','0.0.1',1)")
             .bind(a.node_id()).bind(a.public_key_hex()).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO federation_peer_scope (node_id,tenant_id,stream_id) VALUES ($1,$1,$2)",
        )
        .bind(a.node_id())
        .bind(stream)
        .execute(&pool)
        .await
        .unwrap();

        for (node, kind, duration) in [
            (a.node_id(), "primary", 50_i64),
            (b.node_id(), "secondary", 70),
            (c.node_id(), "secondary", 70),
        ] {
            sqlx::query("INSERT INTO federation_replica_policy
                VALUES ($1,1,$2,$3,1,'BR',$1,$4,'replica_metadata','issued_at',$5,true,'explicit',true)")
                .bind(node).bind(&tenant).bind(CLASS).bind(kind).bind(duration)
                .execute(&pool).await.unwrap();
            if kind == "secondary" {
                for dir in ["export", "import"] {
                    sqlx::query(
                        "INSERT INTO federation_replica_acl
                        VALUES ($1,$1,$2,0,7,$3,$4,$5,1,'BR','secondary',$3,1,0,1000,false)",
                    )
                    .bind(a.node_id())
                    .bind(stream)
                    .bind(node)
                    .bind(dir)
                    .bind(CLASS)
                    .execute(&pool)
                    .await
                    .unwrap();
                }
            }
        }
        let r = Reservation {
            namespace: Namespace {
                origin_node: a.node_id().to_owned(),
                tenant_id: tenant.clone(),
                stream_id: stream.to_owned(),
                partition: 0,
            },
            authority_epoch: 7,
            local_node: a.node_id().to_owned(),
            profile: "metadata".to_owned(),
            profile_version: 1,
            retention_class: CLASS.to_owned(),
            class_version: 1,
            jurisdiction: "BR".to_owned(),
            policy: PolicyRef {
                id: a.node_id().to_owned(),
                version: 1,
            },
        };
        let grant = VerifiedGrant {
            subject: Subject {
                tenant_id: tenant,
                principal: "sintetico".to_owned(),
            },
            token_id: "teste-m4".to_owned(),
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
        Self {
            pool,
            portable,
            a,
            b,
            c,
            r,
            grant,
        }
    }

    fn series(&self) -> Series<'_> {
        Series {
            namespace: &self.r.namespace,
            epoch: self.r.authority_epoch,
        }
    }

    fn event(&self, seq: u64, issued_at: i64) -> Vec<u8> {
        serde_json::to_vec(&signed_event(
            &self.a,
            EventDraft {
                tenant_id: &self.r.namespace.tenant_id,
                stream_id: &self.r.namespace.stream_id,
                partition: 0,
                authority_epoch: 7,
                seq,
                retention_class: CLASS,
                issued_at,
            },
        ))
        .unwrap()
    }

    async fn reserve(&self, now: i64) -> Result<Reserved, Error> {
        let local = [&self.a, &self.b, &self.c]
            .into_iter()
            .find(|id| id.node_id() == self.r.local_node)
            .unwrap();
        let key = local.public_key_hex();
        let mut tx = self.pool.begin().await.unwrap();
        let mut guard = self.portable.lock_cluster_guard(&mut tx).await.unwrap();
        let result =
            Replication::reserve_namespace_in_tx(&mut guard, &self.r, node(local, &key), || now)
                .await;
        tx.commit().await.unwrap();
        result
    }

    async fn counts(&self) -> (i64, i64, i64, i64) {
        sqlx::query_as("SELECT
            (SELECT count(*) FROM event_position WHERE origin_node=$1),
            (SELECT count(*) FROM federation_replica_proof WHERE origin_node=$1),
            (SELECT coalesce(max(prefix),0) FROM federation_replica_checkpoint WHERE origin_node=$1),
            (SELECT count(*) FROM federation_replica_disposition WHERE origin_node=$1)")
            .bind(self.a.node_id()).fetch_one(&self.pool).await.unwrap()
    }
}

fn limits() -> Limits {
    Limits::new(16, 4096, 32768).unwrap()
}

fn node<'a>(id: &'a NodeIdentity, key: &'a str) -> NodeContext<'a> {
    NodeContext {
        node_id: id.node_id(),
        public_key: key,
    }
}

#[tokio::test]
async fn predicado_negado_nao_admite_mesmo_com_escopo_e_tenant() {
    let Some(mut f) = Fixture::new().await else {
        return;
    };
    assert_eq!(f.reserve(100).await.unwrap(), Reserved::Created);
    f.grant.predicate.action_enumerated = false;
    let key = f.a.public_key_hex();
    let mut tx = f.pool.begin().await.unwrap();
    let mut guard = f.portable.lock_cluster_guard(&mut tx).await.unwrap();
    let result = Replication::admit_origin_in_tx(
        &mut guard,
        f.series(),
        OriginAdmission {
            origin: node(&f.a, &key),
            grant: &f.grant,
            max_age_seconds: 10,
            limits: limits(),
        },
        &f.event(1, 100),
        || 100,
    )
    .await;
    tx.commit().await.unwrap();
    assert_eq!(f.counts().await, (0, 0, 0, 0));
    assert!(matches!(result, Err(Error::Authority)), "{result:?}");
}

async fn database(label: &str) -> Option<PgPool> {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    let base: PgConnectOptions = common::database_url()?.parse().unwrap();
    let admin = PgPoolOptions::new()
        .connect_with(base.clone())
        .await
        .unwrap();
    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(version.parse::<u32>().unwrap() / 10_000, 16);
    let db = format!("m4a_{}_{label}", std::process::id());
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {db}")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let pool = PgPoolOptions::new()
        .connect_with(base.database(&db))
        .await
        .unwrap();
    godmesh_server::migrate::migrate(&pool, &common::migrations_dir())
        .await
        .unwrap();
    Some(pool)
}

impl Fixture {
    async fn admit(
        &self,
        raw: &[u8],
        now: i64,
        commit: bool,
    ) -> Result<godmesh_server::replication::Applied, Error> {
        let key = self.a.public_key_hex();
        let mut tx = self.pool.begin().await.unwrap();
        let mut guard = self.portable.lock_cluster_guard(&mut tx).await.unwrap();
        let res = Replication::admit_origin_in_tx(
            &mut guard,
            self.series(),
            OriginAdmission {
                origin: node(&self.a, &key),
                grant: &self.grant,
                max_age_seconds: 10,
                limits: limits(),
            },
            raw,
            || now,
        )
        .await;
        if commit {
            tx.commit().await.unwrap();
        } else {
            tx.rollback().await.unwrap();
        }
        res
    }

    async fn select(
        &self,
        dest: &NodeIdentity,
        first: i64,
        now: i64,
        limits: Limits,
    ) -> Result<godmesh_server::replication::CopyPage, Error> {
        let a = self.a.public_key_hex();
        let b = dest.public_key_hex();
        let mut tx = self.pool.begin().await.unwrap();
        let mut guard = self.portable.lock_cluster_guard(&mut tx).await.unwrap();
        let result = Replication::select_copy_in_tx(
            &mut guard,
            self.series(),
            CopyAuthority {
                origin: node(&self.a, &a),
                destination: node(dest, &b),
                limits,
            },
            first,
            || now,
        )
        .await;
        tx.commit().await.unwrap();
        result
    }

    async fn apply(
        &self,
        dest: &NodeIdentity,
        frames: &[Vec<u8>],
        now: i64,
    ) -> Result<godmesh_server::replication::Applied, Error> {
        let a = self.a.public_key_hex();
        let b = dest.public_key_hex();
        let mut tx = self.pool.begin().await.unwrap();
        let mut guard = self.portable.lock_cluster_guard(&mut tx).await.unwrap();
        let result = Replication::apply_copy_in_tx(
            &mut guard,
            self.series(),
            CopyAuthority {
                origin: node(&self.a, &a),
                destination: node(dest, &b),
                limits: limits(),
            },
            frames,
            || now,
        )
        .await;
        tx.commit().await.unwrap();
        result
    }

    async fn discard(&self, seq: i64, now: i64, commit: bool) -> Result<Disposal, Error> {
        let key = self.a.public_key_hex();
        let mut tx = self.pool.begin().await.unwrap();
        let mut guard = self.portable.lock_cluster_guard(&mut tx).await.unwrap();
        let result = Replication::discard_due_in_tx(
            &mut guard,
            self.series(),
            node(&self.a, &key),
            seq,
            || now,
        )
        .await;
        if commit {
            tx.commit().await.unwrap();
        } else {
            tx.rollback().await.unwrap();
        }
        result
    }
}

#[tokio::test]
async fn reserva_nova_recusa_historico_em_epoch_distinto() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    sqlx::query("INSERT INTO event_position
         (origin_node,tenant_id,stream_id,partition,authority_epoch,seq,retention_class,jurisdiction)
         VALUES ($1,$1,$2,0,9,1,$3,'BR')")
        .bind(f.a.node_id()).bind(&f.r.namespace.stream_id).bind(CLASS).execute(&f.pool).await.unwrap();
    assert!(matches!(f.reserve(100).await, Err(Error::LegacyHistory)));
    assert_eq!(f.counts().await, (1, 0, 0, 0));
}

#[tokio::test]
async fn origem_reverte_e_replay_nao_reaplica_janela() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    assert_eq!(f.reserve(100).await.unwrap(), Reserved::Created);
    let raw = f.event(1, 100);
    assert!(matches!(
        f.admit(&raw, 111, true).await,
        Err(Error::NotFresh)
    ));
    assert_eq!(f.counts().await, (0, 0, 0, 0));
    assert_eq!(f.admit(&raw, 100, false).await.unwrap().inserted, 1);
    assert_eq!(f.counts().await, (0, 0, 0, 0));
    f.admit(&raw, 100, true).await.unwrap();
    assert_eq!(f.reserve(100).await.unwrap(), Reserved::Replay);
    assert_eq!(f.admit(&raw, 140, true).await.unwrap().inserted, 0);
    assert!(matches!(
        f.admit(&f.event(1, 101), 140, true).await,
        Err(Error::ProofConflict)
    ));
    let stored: (Vec<u8>, i64, bool) = sqlx::query_as(
        "SELECT frame,expires_at,published FROM federation_replica_proof WHERE origin_node=$1",
    )
    .bind(f.a.node_id())
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(stored, (raw, 150, true));
}

#[tokio::test]
async fn copia_a_b_c_tem_prefixos_independentes_e_replay_exato() {
    let Some(a_pool) = database("a").await else {
        return;
    };
    let a = Fixture::seed(
        a_pool,
        Arc::new(NodeIdentity::generate_in_memory().unwrap()),
        Arc::new(NodeIdentity::generate_in_memory().unwrap()),
        Arc::new(NodeIdentity::generate_in_memory().unwrap()),
    )
    .await;
    let mut b = Fixture::seed(
        database("b").await.unwrap(),
        a.a.clone(),
        a.b.clone(),
        a.c.clone(),
    )
    .await;
    let mut c = Fixture::seed(
        database("c").await.unwrap(),
        a.a.clone(),
        a.b.clone(),
        a.c.clone(),
    )
    .await;
    for (f, id) in [(&mut b, &a.b), (&mut c, &a.c)] {
        f.r.local_node = id.node_id().to_owned();
        f.r.policy.id = id.node_id().to_owned();
        assert_eq!(f.reserve(100).await.unwrap(), Reserved::Created);
    }
    assert_eq!(a.reserve(100).await.unwrap(), Reserved::Created);
    let frames = [a.event(1, 100), a.event(2, 101)];
    for raw in &frames {
        a.admit(raw, 101, true).await.unwrap();
    }
    let first = a
        .select(
            &a.b,
            1,
            135,
            Limits::new(1, frames[0].len(), frames[0].len()).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.frames, [frames[0].clone()]);
    assert!(first.has_more);
    assert!(matches!(
        a.select(
            &a.b,
            1,
            135,
            Limits::new(1, frames[0].len() - 1, frames[0].len()).unwrap()
        )
        .await,
        Err(Error::LimitExceeded)
    ));
    assert!(
        a.select(
            &a.b,
            1,
            135,
            Limits::new(2, 4096, frames[0].len() + frames[1].len() - 1).unwrap()
        )
        .await
        .unwrap()
        .has_more
    );
    assert!(matches!(
        b.apply(&a.b, &frames[1..], 135).await,
        Err(Error::SequenceGap)
    ));
    let corrupt = [frames[0].clone(), b"{}".to_vec()];
    assert!(matches!(
        b.apply(&a.b, &corrupt, 135).await,
        Err(Error::InvalidProof)
    ));
    assert_eq!(b.counts().await, (0, 0, 0, 0));
    b.apply(&a.b, &first.frames, 135).await.unwrap();
    assert_eq!(c.counts().await, (0, 0, 0, 0));
    assert_eq!(b.apply(&a.b, &first.frames, 136).await.unwrap().inserted, 0);
    assert_eq!(b.apply(&a.b, &frames, 136).await.unwrap().inserted, 1);
    assert_eq!(b.counts().await, (2, 2, 2, 0));

    sqlx::query("UPDATE federation_replica_acl SET revoked=true WHERE destination=$1 AND direction='export'")
        .bind(a.b.node_id()).execute(&a.pool).await.unwrap();
    assert!(matches!(
        a.select(&a.b, 1, 135, limits()).await,
        Err(Error::AclDenied)
    ));
    let page = a.select(&a.c, 1, 135, limits()).await.unwrap();
    assert_eq!(page.frames, frames);
    c.apply(&a.c, &page.frames, 135).await.unwrap();
    assert_eq!(c.counts().await, (2, 2, 2, 0));
    sqlx::query("DELETE FROM federation_replica_proof WHERE origin_node=$1 AND seq=1")
        .bind(a.a.node_id())
        .execute(&c.pool)
        .await
        .unwrap();
    assert!(matches!(
        c.apply(&a.c, &page.frames, 135).await,
        Err(Error::ProofMissing)
    ));
}

#[tokio::test]
async fn descarte_historico_com_hold_e_expiracao_fora_da_ordem() {
    let Some(mut f) = Fixture::new().await else {
        return;
    };
    f.reserve(100).await.unwrap();
    f.admit(&f.event(1, 105), 100, true).await.unwrap();
    f.admit(&f.event(2, 100), 100, true).await.unwrap();
    assert_eq!(f.discard(2, 149, true).await.unwrap(), Disposal::NotDue);
    assert!(matches!(
        f.select(&f.b, 2, 150, limits()).await,
        Err(Error::HistoryUnavailable)
    ));
    assert!(matches!(
        f.discard(2, 150, true).await,
        Err(Error::HoldUnknown)
    ));

    let origin = f.a.node_id();
    sqlx::query(
        "INSERT INTO federation_replica_hold VALUES ($1,$1,$2,0,7,1,true),($1,$1,$2,0,7,2,true)",
    )
    .bind(origin)
    .bind(&f.r.namespace.stream_id)
    .execute(&f.pool)
    .await
    .unwrap();
    assert!(matches!(
        f.discard(2, 150, true).await,
        Err(Error::HoldActive)
    ));

    sqlx::query("UPDATE federation_replica_policy SET active=false WHERE policy_id=$1")
        .bind(origin)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO federation_replica_policy
       SELECT policy_id,2,tenant_id,retention_class,class_version,jurisdiction,copy_node,copy_kind,
              purpose,trigger_event,900,allow_discard,hold_mode,true
       FROM federation_replica_policy WHERE policy_id=$1 AND policy_version=1",
    )
    .bind(origin)
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE federation_replica_acl SET revoked=true WHERE origin_node=$1")
        .bind(origin)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE federation_replica_hold SET active=false WHERE origin_node=$1")
        .bind(origin)
        .execute(&f.pool)
        .await
        .unwrap();
    assert_eq!(f.discard(2, 150, false).await.unwrap(), Disposal::Removed);
    assert_eq!(f.counts().await, (2, 2, 2, 0));
    assert_eq!(f.discard(2, 150, true).await.unwrap(), Disposal::Removed);
    assert_eq!(
        f.discard(2, 151, true).await.unwrap(),
        Disposal::AlreadyRemoved
    );
    sqlx::query(
        "UPDATE federation_replica_policy SET active=false WHERE policy_id=$1 AND policy_version=2",
    )
    .bind(origin)
    .execute(&f.pool)
    .await
    .unwrap();
    assert_eq!(f.discard(1, 154, true).await.unwrap(), Disposal::NotDue);
    assert_eq!(f.discard(1, 155, true).await.unwrap(), Disposal::Removed);
    assert_eq!(f.counts().await, (0, 0, 2, 2));
    assert_eq!(f.reserve(300).await.unwrap(), Reserved::Replay);
    f.r.authority_epoch = 9;
    assert!(matches!(
        f.reserve(300).await,
        Err(Error::ReservationConflict)
    ));
}

#[tokio::test]
async fn politica_e_acl_ficam_protegidas_ate_o_commit() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.reserve(100).await.unwrap();
    f.admit(&f.event(1, 100), 100, true).await.unwrap();
    let a_key = f.a.public_key_hex();
    let b_key = f.b.public_key_hex();
    let mut selected = f.pool.begin().await.unwrap();
    let mut guard = f.portable.lock_cluster_guard(&mut selected).await.unwrap();
    let copy = Replication::select_copy_in_tx(
        &mut guard,
        f.series(),
        CopyAuthority {
            origin: node(&f.a, &a_key),
            destination: node(&f.b, &b_key),
            limits: limits(),
        },
        1,
        || 100,
    )
    .await
    .unwrap();
    assert_eq!(copy.frames.len(), 1);

    for (sql, id) in [
        (
            "UPDATE federation_replica_acl SET revoked=true WHERE destination=$1",
            f.b.node_id(),
        ),
        (
            "UPDATE federation_replica_policy SET active=false WHERE policy_id=$1",
            f.a.node_id(),
        ),
        (
            "UPDATE federation_replica_policy SET active=false WHERE policy_id=$1",
            f.b.node_id(),
        ),
        (
            "UPDATE tenant_region SET jurisdiction='CO' WHERE tenant_id=$1",
            f.a.node_id(),
        ),
    ] {
        let mut other = f.pool.begin().await.unwrap();
        sqlx::query("SET LOCAL lock_timeout='25ms'")
            .execute(&mut *other)
            .await
            .unwrap();
        let err = sqlx::query(sql)
            .bind(id)
            .execute(&mut *other)
            .await
            .unwrap_err();
        assert_eq!(
            err.as_database_error().unwrap().code().as_deref(),
            Some("55P03")
        );
        other.rollback().await.unwrap();
    }
    selected.commit().await.unwrap();

    sqlx::query("UPDATE federation_replica_acl SET revoked=true WHERE destination=$1")
        .bind(f.b.node_id())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(
        f.select(&f.b, 1, 100, limits()).await,
        Err(Error::AclDenied)
    ));
    assert_eq!(
        f.select(&f.c, 1, 100, limits()).await.unwrap().frames,
        copy.frames
    );
}

#[tokio::test]
async fn politica_inconsistente_nao_remove_nem_grava_disposicao() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.reserve(100).await.unwrap();
    f.admit(&f.event(1, 100), 100, true).await.unwrap();
    sqlx::query("INSERT INTO federation_replica_hold VALUES ($1,$1,$2,0,7,1,false)")
        .bind(f.a.node_id())
        .bind(&f.r.namespace.stream_id)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE federation_replica_proof SET issued_at=99, expires_at=149 WHERE origin_node=$1",
    )
    .bind(f.a.node_id())
    .execute(&f.pool)
    .await
    .unwrap();
    assert!(matches!(
        f.discard(1, 150, true).await,
        Err(Error::ProofConflict)
    ));
    sqlx::query("DELETE FROM federation_replica_policy WHERE policy_id=$1")
        .bind(f.a.node_id())
        .execute(&f.pool)
        .await
        .unwrap();
    assert!(matches!(f.discard(1, 150, true).await, Err(Error::Policy)));
    assert_eq!(f.counts().await, (1, 1, 1, 0));
}

#[tokio::test]
async fn criacao_exige_politica_e_acl_da_tupla_exata() {
    let Some(mut f) = Fixture::new().await else {
        return;
    };
    f.r.retention_class = "mensagem".to_owned();
    assert!(matches!(f.reserve(100).await, Err(Error::Policy)));
    f.r.retention_class = CLASS.to_owned();
    f.r.jurisdiction = "CO".to_owned();
    assert!(matches!(f.reserve(100).await, Err(Error::Policy)));
    f.r.jurisdiction = "BR".to_owned();
    assert!(matches!(f.reserve(1000).await, Err(Error::AclDenied)));
    sqlx::query("UPDATE federation_replica_acl SET class_version=2 WHERE origin_node=$1 AND direction='export'")
        .bind(f.a.node_id()).execute(&f.pool).await.unwrap();
    assert!(matches!(f.reserve(100).await, Err(Error::AclDenied)));
    sqlx::query("UPDATE federation_replica_acl SET class_version=1 WHERE origin_node=$1 AND direction='export'")
        .bind(f.a.node_id()).execute(&f.pool).await.unwrap();
    assert_eq!(f.reserve(100).await.unwrap(), Reserved::Created);

    let unknown = NodeIdentity::generate_in_memory().unwrap();
    assert!(matches!(
        f.select(&unknown, 1, 100, limits()).await,
        Err(Error::AclDenied)
    ));
    assert_eq!(f.counts().await, (0, 0, 0, 0));
}
