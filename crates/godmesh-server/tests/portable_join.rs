mod common;

async fn empty_pool(suffix: &str) -> Option<sqlx::PgPool> {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    let base = common::database_url()?.parse::<PgConnectOptions>().unwrap();
    let admin = PgPoolOptions::new()
        .connect_with(base.clone().database("postgres"))
        .await
        .unwrap();
    let name = format!("m1cb_{}_{suffix}", std::process::id());
    assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
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

async fn isolated_pool(suffix: &str) -> Option<sqlx::PgPool> {
    let pool = empty_pool(suffix).await?;
    godmesh_server::migrate::migrate(&pool, &common::migrations_dir())
        .await
        .unwrap();
    Some(pool)
}

#[tokio::test]
async fn migracao_portatil_e_aditiva_e_idempotente() {
    let Some(pool) = empty_pool("migration").await else {
        return;
    };
    let old = std::env::temp_dir().join(format!("godmesh-m1cb-migrations-{}", std::process::id()));
    std::fs::create_dir(&old).unwrap();
    for file in std::fs::read_dir(common::migrations_dir()).unwrap() {
        let file = file.unwrap();
        let name = file.file_name();
        if file.path().extension().is_some_and(|ext| ext == "sql")
            && !name.to_string_lossy().starts_with("0012")
        {
            std::fs::copy(file.path(), old.join(name)).unwrap();
        }
    }
    let legacy = godmesh_server::migrate::migrate(&pool, &old).await.unwrap();
    assert_eq!(legacy.applied.len(), 11);
    std::fs::remove_dir_all(old).unwrap();
    let approved_legacy_id = format!("gmn_{}", "1".repeat(64));
    sqlx::query(
        "INSERT INTO federation_decision_epoch (approver_node_id, decision_epoch) VALUES ($1, 17)",
    )
    .bind(&approved_legacy_id)
    .execute(&pool)
    .await
    .unwrap();
    let history: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT nome, checksum, aplicado_em::text FROM schema_migration ORDER BY nome",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        history.last().unwrap().0,
        "0011_federation_decision_epoch.sql"
    );
    let upgrade = godmesh_server::migrate::migrate(&pool, &common::migrations_dir())
        .await
        .unwrap();
    assert_eq!(upgrade.applied, ["0012_federation_portable_join.sql"]);
    assert_eq!(upgrade.skipped_count(), 11);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM federation_portable_roots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let repeat = godmesh_server::migrate::migrate(&pool, &common::migrations_dir())
        .await
        .unwrap();
    assert!(repeat.applied.is_empty());
    assert_eq!(repeat.skipped_count(), 12);
    let after: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT nome, checksum, aplicado_em::text FROM schema_migration ORDER BY nome",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(history, &after[..11]);
    assert_eq!(after.last().unwrap().0, "0012_federation_portable_join.sql");
    let legacy_floor: i64 = sqlx::query_scalar(
        "SELECT decision_epoch FROM federation_decision_epoch WHERE approver_node_id = $1",
    )
    .bind(&approved_legacy_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_floor, 17);
}

use std::sync::Arc;

use godmesh_federation::NodeIdentity;
use godmesh_server::portable_join::{PortableJoin, PortableJoinConfig, PortableJoinError};

fn configuration(root: &NodeIdentity) -> std::collections::HashMap<&'static str, String> {
    [
        (
            "GODMESH_FEDERATION_GOVERNANCE",
            "single_approver:test-portable".into(),
        ),
        (
            "GODMESH_FEDERATION_GENESIS",
            format!("{}:{}", root.node_id(), root.public_key_hex()),
        ),
        ("GODMESH_FEDERATION_REGION", "lab".into()),
        (
            "GODMESH_FEDERATION_ADVERTISE_QUIC",
            "quic://[::1]:4433".into(),
        ),
        ("GODMESH_FEDERATION_PORTABLE_MAX_DEPTH", "4".into()),
        ("GODMESH_FEDERATION_PORTABLE_MAX_FRAME_BYTES", "8192".into()),
        (
            "GODMESH_FEDERATION_PORTABLE_MAX_VALIDITY_SECONDS",
            "60".into(),
        ),
    ]
    .into()
}

#[test]
fn configuracao_parcial_e_endereco_nao_publicavel_falham() {
    let root = NodeIdentity::generate_in_memory().unwrap();
    let mut env = configuration(&root);
    assert!(
        PortableJoinConfig::from_env_with(|k| env.get(k).cloned())
            .unwrap()
            .is_some()
    );
    env.insert("GODMESH_FEDERATION_CLUSTER", "different-cluster".into());
    assert!(matches!(
        PortableJoinConfig::from_env_with(|k| env.get(k).cloned()),
        Err(PortableJoinError::Configuration(
            "GODMESH_FEDERATION_CLUSTER"
        ))
    ));
    env.remove("GODMESH_FEDERATION_CLUSTER");
    for endpoint in [
        "quic://[::]:1",
        "quic://224.0.0.1:23",
        "quic://127.0.0.1:0",
        "quic://localhost:4433",
        "quic://127.0.0.1:4433/",
        "quic://[::1%en0]:4433",
        "quic://127.0.0.1:+1",
    ] {
        env.insert("GODMESH_FEDERATION_ADVERTISE_QUIC", endpoint.into());
        assert!(
            PortableJoinConfig::from_env_with(|k| env.get(k).cloned()).is_err(),
            "{endpoint}"
        );
    }
    env.remove("GODMESH_FEDERATION_ADVERTISE_QUIC");
    assert!(PortableJoinConfig::from_env_with(|k| env.get(k).cloned()).is_err());
    assert!(
        PortableJoinConfig::from_env_with(|_| None)
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn preflight_e_autorizacao_distinguem_pin_local_de_par_importado() {
    let Some(pool) = isolated_pool("preflight").await else {
        return;
    };
    let root = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let env = configuration(&root);
    let config = PortableJoinConfig::from_env_with(|k| env.get(k).cloned())
        .unwrap()
        .unwrap();
    let svc = PortableJoin::new(config.clone(), pool.clone(), root.clone());
    PortableJoin::preflight(&pool, None).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(svc.authorize_local(&mut tx).await.unwrap(), None);
    assert!(
        svc.authorize_peer(&mut tx, root.node_id(), &root.public_key_hex())
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO federation_portable_roots VALUES ($1, $2, $3)")
        .bind(svc.cluster())
        .bind(root.node_id())
        .bind(root.public_key_hex())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(
        PortableJoin::preflight(&pool, None).await,
        Err(PortableJoinError::Configuration(_))
    ));
    PortableJoin::preflight(&pool, Some(&config)).await.unwrap();
    sqlx::query("DELETE FROM federation_portable_roots WHERE root_node_id = $1")
        .bind(root.node_id())
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn oferta_confirmacao_vigencia_importacao_e_revogacao() {
    use godmesh_federation::join_authority::JoinRevocationV1;
    use godmesh_federation::join_chain::JoinChainV2;
    use godmesh_federation::now_unix;
    use godmesh_server::federation::{JoinWindow, submit_join_request};
    use godmesh_server::portable_join::JoinSignatureRequest;
    let Some(pool) = isolated_pool("flow").await else {
        return;
    };
    let root = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let candidate = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let mut env = configuration(&root);
    env.insert(
        "GODMESH_FEDERATION_GOVERNANCE",
        "single_approver:flow-test".into(),
    );
    let config = PortableJoinConfig::from_env_with(|k| env.get(k).cloned())
        .unwrap()
        .unwrap();
    let issuer = PortableJoin::new(config.clone(), pool.clone(), root.clone());
    let member = PortableJoin::new(config, pool.clone(), candidate);
    let manifest = member.local_manifest().unwrap();
    let now = now_unix();
    let pending = submit_join_request(&pool, JoinWindow::default(), &manifest, now)
        .await
        .unwrap()
        .request;
    let offer = issuer
        .sign_offer(
            JoinSignatureRequest {
                join_request_id: pending.join_request_id.clone(),
                manifest: manifest.clone(),
                can_approve: true,
                not_before: now,
                expires_at: now + 600,
                epoch: 5,
            },
            JoinWindow::default(),
            16384,
        )
        .await
        .unwrap();
    assert!(matches!(
        issuer.proof(&pending.join_request_id).await,
        Err(PortableJoinError::NotFound)
    ));
    assert!(matches!(
        issuer
            .approve(
                &pending.join_request_id,
                &offer,
                "tester",
                JoinWindow::default(),
                offer.len()
            )
            .await,
        Err(PortableJoinError::Capacity)
    ));
    assert!(
        issuer
            .approve(
                &pending.join_request_id,
                &offer,
                "tester",
                JoinWindow::default(),
                16384
            )
            .await
            .unwrap()
            .0
    );
    assert!(
        !issuer
            .approve(
                &pending.join_request_id,
                &offer,
                "tester",
                JoinWindow::default(),
                16384
            )
            .await
            .unwrap()
            .0
    );
    let approved = godmesh_server::federation::list_join_requests(&pool, Some("approved"))
        .await
        .unwrap();
    assert_eq!(approved.len(), 1);
    assert_eq!(approved[0].join_request_id, pending.join_request_id);
    assert_eq!(approved[0].approved_by.as_deref(), Some("tester"));
    assert!(approved[0].approval_signature.is_none());
    let validity = issuer.issue_validity(&offer).await.unwrap();
    assert!(
        member
            .import_chain(&manifest.node_id, &manifest.public_key, &offer, &validity)
            .await
            .unwrap()
    );
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        member.authorize_local(&mut tx).await.unwrap(),
        Some(validity.expires_at)
    );
    tx.rollback().await.unwrap();
    let chain: JoinChainV2 = serde_json::from_str(&offer).unwrap();
    let mut revoke = JoinRevocationV1 {
        cluster: issuer.cluster().into(),
        root_node_id: root.node_id().into(),
        target_digest: chain.links.last().unwrap().certificate.digest().unwrap(),
        revoker_node_id: String::new(),
        epoch: 5,
        signature: String::new(),
    };
    revoke.sign(&root);
    let watch = issuer.trust_changes();
    let (created, receipt) = issuer.revoke(&offer, &revoke).await.unwrap();
    assert!(created && watch.has_changed().unwrap());
    assert!(matches!(
        issuer.issue_validity(&offer).await,
        Err(PortableJoinError::Revoked)
    ));
    assert!(
        !member
            .import_revocation(&offer, &revoke, &receipt)
            .await
            .unwrap()
    );
    assert_eq!(issuer.proof(&pending.join_request_id).await.unwrap(), offer);
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        member.authorize_local(&mut tx).await,
        Err(PortableJoinError::Revoked)
    ));
}

async fn serve_portable(
    portable: Arc<PortableJoin>,
    admission: godmesh_server::admission::AdmissionConfig,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
    use godmesh_server::auth::{GrantRecord, GrantRegistry, Scope, Subject};
    use godmesh_server::federation::DecisionConfig;
    use godmesh_server::http::{HostTelemetry, router_runtime_with_portable};

    let grant = |token: &str, scope: Scope| {
        (
            token.to_owned(),
            GrantRecord {
                token_id: token.to_owned(),
                subject: Subject {
                    tenant_id: "portable-http".into(),
                    principal: token.into(),
                },
                scopes: vec![scope],
                not_before: 0,
                expires_at: i64::MAX,
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
            },
        )
    };
    let grants = GrantRegistry::from_records([
        grant("admin-test", Scope::FederationAdmin),
        grant("read-test", Scope::FederationRead),
    ]);
    let router = router_runtime_with_portable(
        portable.pool().clone(),
        grants,
        None,
        admission,
        false,
        HostTelemetry::default(),
        Arc::new(DecisionConfig::default()),
        Some(portable),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let runner = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (addr, runner)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_aplica_escopo_envelope_quota_e_frame() {
    use godmesh_server::admission::AdmissionConfig;
    use serde_json::json;

    let Some(pool) = isolated_pool("http").await else {
        return;
    };
    let root = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let mut env = configuration(&root);
    env.insert("GODMESH_FEDERATION_PORTABLE_MAX_FRAME_BYTES", "192".into());
    let config = PortableJoinConfig::from_env_with(|k| env.get(k).cloned())
        .unwrap()
        .unwrap();
    let portable = Arc::new(PortableJoin::new(config, pool.clone(), root));
    let (addr, runner) = serve_portable(
        portable,
        AdmissionConfig {
            max_body_bytes: 512,
            ..AdmissionConfig::default()
        },
    )
    .await;
    tokio::task::spawn_blocking(move || {
        let path = "/v1/federation/local-manifest";
        assert_eq!(common::call(addr, "GET", path, None, None).status, 401);
        assert_eq!(common::get(addr, path, "read-test").status, 403);
        let manifest = common::get(addr, path, "admin-test");
        assert_eq!(manifest.status, 200, "{}", manifest.body);
        assert_eq!(
            manifest.json()["manifest"]["endpoints"],
            json!(["quic://[::1]:4433"])
        );
        let call = |raw: &str| {
            common::call(
                addr,
                "POST",
                "/v1/federation/join-validity",
                Some("admin-test"),
                Some(raw),
            )
        };
        assert_eq!(
            call(r#"{"chain_json":{}}"#).json()["error"],
            "invalid_request"
        );
        assert_eq!(
            call(r#"{"chain_json":"{}","chain_json":"{}"}"#).json()["error"],
            "invalid_request"
        );
        assert_eq!(call(r#"["{}"]"#).json()["error"], "invalid_request");
        let interior = json!({"chain_json": format!("{}{}", " ".repeat(190), "{}")}).to_string();
        let admitted = format!("{interior}{}", " ".repeat(512 - interior.len()));
        let reply = call(&admitted);
        assert_eq!(reply.status, 422);
        assert_ne!(reply.json()["reason"], "frame_too_large");
        let reply = call(&(admitted + " "));
        assert_eq!(reply.status, 413);
        assert_eq!(reply.json()["error"], "payload_too_large");
        let internal = json!({"chain_json": "\"".repeat(193)}).to_string();
        assert!(internal.len() < 512 && internal.len() > 193);
        let reply = call(&internal);
        assert_eq!(reply.status, 422);
        assert_eq!(reply.json()["reason"], "frame_too_large");
        let duplicate =
            json!({"chain_json": r#"{"version":2,"version":2,"links":[]}"#}).to_string();
        let reply = call(&duplicate);
        assert_eq!(reply.status, 422);
        assert_eq!(reply.json()["reason"], "malformed_frame");
    })
    .await
    .unwrap();
    runner.abort();
}

#[tokio::test]
async fn delegacao_e_recibo_tardio_bloqueiam_descendentes_e_permitem_cadeia_alternativa() {
    use godmesh_federation::join_authority::{JoinRevocationV1, JoinValidityV1};
    use godmesh_federation::join_chain::JoinChainV2;
    use godmesh_federation::{SignedManifest, now_unix};
    use godmesh_server::federation::{JoinWindow, submit_join_request};
    use godmesh_server::portable_join::JoinSignatureRequest;

    let Some(a_db) = isolated_pool("delegation_a").await else {
        return;
    };
    let b_db = isolated_pool("delegation_b").await.unwrap();
    let c_db = isolated_pool("delegation_c").await.unwrap();
    let root = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let signer = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let child = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let alt_child = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let env = configuration(&root);
    let config = PortableJoinConfig::from_env_with(|k| env.get(k).cloned())
        .unwrap()
        .unwrap();
    let a = PortableJoin::new(config.clone(), a_db.clone(), root.clone());
    let b = PortableJoin::new(config.clone(), b_db.clone(), signer.clone());
    let c = PortableJoin::new(config.clone(), c_db.clone(), child.clone());
    let d = PortableJoin::new(config.clone(), c_db.clone(), alt_child);
    let window = JoinWindow::default();
    let now = now_unix();
    let root_manifest = a.local_manifest().unwrap();
    assert!(b.import_genesis(&root_manifest, window).await.unwrap());
    assert!(!b.import_genesis(&root_manifest, window).await.unwrap());
    assert!(matches!(
        b.import_genesis(&c.local_manifest().unwrap(), window).await,
        Err(PortableJoinError::Issuer)
    ));
    let mut tx = b_db.begin().await.unwrap();
    assert_eq!(
        b.authorize_peer(&mut tx, root.node_id(), &root.public_key_hex())
            .await
            .unwrap(),
        None
    );
    tx.rollback().await.unwrap();
    sqlx::query("UPDATE federation_peer SET trust_state = 'revoked' WHERE node_id = $1")
        .bind(root.node_id())
        .execute(&b_db)
        .await
        .unwrap();
    let mut refreshed = root_manifest.clone();
    refreshed.issued_at += 1;
    refreshed.sign(&root);
    assert!(!b.import_genesis(&refreshed, window).await.unwrap());
    let mut tx = b_db.begin().await.unwrap();
    assert!(matches!(
        b.authorize_peer(&mut tx, root.node_id(), &root.public_key_hex())
            .await,
        Err(PortableJoinError::Issuer)
    ));
    tx.rollback().await.unwrap();

    async fn pending(root: &PortableJoin, manifest: &SignedManifest) -> String {
        submit_join_request(root.pool(), JoinWindow::default(), manifest, now_unix())
            .await
            .unwrap()
            .request
            .join_request_id
    }
    let request = |id: String, manifest: SignedManifest, epoch, can_approve| JoinSignatureRequest {
        join_request_id: id,
        manifest,
        epoch,
        can_approve,
        not_before: now,
        expires_at: now + 900,
    };
    let b_manifest = b.local_manifest().unwrap();
    let b_id = pending(&a, &b_manifest).await;
    let b_offer = a
        .sign_offer(
            request(b_id.clone(), b_manifest.clone(), 8, true),
            window,
            16384,
        )
        .await
        .unwrap();
    assert!(matches!(
        a.issue_validity(&b_offer).await,
        Err(PortableJoinError::Issuer)
    ));
    sqlx::query(
        "INSERT INTO federation_decision_epoch (approver_node_id, decision_epoch) VALUES ($1, 80)",
    )
    .bind(root.node_id())
    .execute(&a_db)
    .await
    .unwrap();
    a.approve(&b_id, &b_offer, "operator", window, 16384)
        .await
        .unwrap();
    let lease = a.issue_validity(&b_offer).await.unwrap();
    assert!(
        b.import_chain(signer.node_id(), &signer.public_key_hex(), &b_offer, &lease)
            .await
            .unwrap()
    );
    let self_peers: i64 =
        sqlx::query_scalar("SELECT count(*) FROM federation_peer WHERE node_id = $1")
            .bind(signer.node_id())
            .fetch_one(&b_db)
            .await
            .unwrap();
    assert_eq!(self_peers, 0);

    let c_manifest = c.local_manifest().unwrap();
    let c_id = pending(&a, &c_manifest).await;
    let delegated = b
        .sign_offer(
            request(c_id.clone(), c_manifest.clone(), 20, false),
            window,
            16384,
        )
        .await
        .unwrap();
    let (decisions, floors): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM federation_portable_decisions), (SELECT count(*) FROM federation_portable_epoch)",
    ).fetch_one(&b_db).await.unwrap();
    assert_eq!((decisions, floors), (0, 0));
    assert!(matches!(
        a.approve(&b_id, &delegated, "operator", window, 16384)
            .await,
        Err(PortableJoinError::Proof("request_binding"))
    ));
    let mut changed = c_manifest.clone();
    changed.region.push_str("-changed");
    changed.sign(&child);
    let mismatched = b
        .sign_offer(request(c_id.clone(), changed, 20, false), window, 16384)
        .await
        .unwrap();
    assert!(matches!(
        a.approve(&c_id, &mismatched, "operator", window, 16384)
            .await,
        Err(PortableJoinError::Proof("request_binding"))
    ));
    a.approve(&c_id, &delegated, "operator", window, 16384)
        .await
        .unwrap();
    let c_lease = a.issue_validity(&delegated).await.unwrap();
    assert!(matches!(
        c.import_chain(child.node_id(), &child.public_key_hex(), &delegated, &lease)
            .await,
        Err(PortableJoinError::Validity)
    ));
    assert!(
        c.import_chain(
            child.node_id(),
            &child.public_key_hex(),
            &delegated,
            &c_lease
        )
        .await
        .unwrap()
    );

    let d_manifest = d.local_manifest().unwrap();
    let d_id = pending(&a, &d_manifest).await;
    let regressed = b
        .sign_offer(
            request(d_id.clone(), d_manifest.clone(), 19, false),
            window,
            16384,
        )
        .await
        .unwrap();
    assert!(matches!(
        a.approve(&d_id, &regressed, "operator", window, 16384)
            .await,
        Err(PortableJoinError::Epoch {
            recorded: 20,
            presented: 19
        })
    ));
    assert!(matches!(
        a.proof(&d_id).await,
        Err(PortableJoinError::NotFound)
    ));
    let sibling = b
        .sign_offer(request(d_id.clone(), d_manifest, 21, false), window, 16384)
        .await
        .unwrap();
    a.approve(&d_id, &sibling, "operator", window, 16384)
        .await
        .unwrap();
    let d_lease = a.issue_validity(&sibling).await.unwrap();

    let make_revocation = |raw: &str, identity: &NodeIdentity, epoch| {
        let chain: JoinChainV2 = serde_json::from_str(raw).unwrap();
        let mut obj = JoinRevocationV1 {
            cluster: a.cluster().into(),
            root_node_id: root.node_id().into(),
            target_digest: chain.links.last().unwrap().certificate.digest().unwrap(),
            revoker_node_id: String::new(),
            epoch,
            signature: String::new(),
        };
        obj.sign(identity);
        obj
    };
    let child_revoke = make_revocation(&sibling, &signer, 22);
    let (_, child_receipt) = a.revoke(&sibling, &child_revoke).await.unwrap();
    assert!(
        d.import_revocation(&sibling, &child_revoke, &child_receipt)
            .await
            .unwrap()
    );
    let d_manifest = d.local_manifest().unwrap();
    assert!(matches!(
        d.import_chain(
            &d_manifest.node_id,
            &d_manifest.public_key,
            &sibling,
            &d_lease
        )
        .await,
        Err(PortableJoinError::Revoked)
    ));

    let mut alternative = c_manifest;
    alternative.epoch += 1;
    alternative.sign(&child);
    let id = pending(&a, &alternative).await;
    assert!(
        a.import_chain(signer.node_id(), &signer.public_key_hex(), &b_offer, &lease)
            .await
            .unwrap()
    );
    sqlx::query("UPDATE federation_peer SET trust_state = 'revoked' WHERE node_id = $1")
        .bind(signer.node_id())
        .execute(&a_db)
        .await
        .unwrap();
    let offer_from_revoked = b
        .sign_offer(
            request(id.clone(), alternative.clone(), 23, false),
            window,
            16384,
        )
        .await
        .unwrap();
    let new_approval = a
        .approve(&id, &offer_from_revoked, "operator", window, 16384)
        .await;
    let revoke_from_revoked = make_revocation(&delegated, &signer, 23);
    let new_revocation = a.revoke(&delegated, &revoke_from_revoked).await;
    assert!(
        matches!(
            (&new_approval, &new_revocation),
            (
                Err(PortableJoinError::Issuer),
                Err(PortableJoinError::Issuer)
            )
        ),
        "aprovação: {new_approval:?}, revogação: {new_revocation:?}"
    );
    assert!(matches!(
        a.proof(&id).await,
        Err(PortableJoinError::NotFound)
    ));
    let epoch: i64 = sqlx::query_scalar(
        "SELECT epoch FROM federation_portable_epoch WHERE cluster = $1 AND root_node_id = $2 AND signer_node_id = $3"
    ).bind(a.cluster()).bind(root.node_id()).bind(signer.node_id()).fetch_one(&a_db).await.unwrap();
    assert_eq!(epoch, 22);
    let revoked_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM federation_portable_revocations WHERE target_digest = $1",
    )
    .bind(&revoke_from_revoked.target_digest)
    .fetch_one(&a_db)
    .await
    .unwrap();
    assert_eq!(revoked_count, 0);
    assert_eq!(
        a.approve(&c_id, &delegated, "operator", window, 16384)
            .await
            .unwrap(),
        (false, delegated.clone())
    );
    assert_eq!(
        a.revoke(&sibling, &child_revoke).await.unwrap(),
        (false, child_receipt.clone())
    );

    let revoke = make_revocation(&b_offer, &root, 9);
    let (_, receipt) = a.revoke(&b_offer, &revoke).await.unwrap();
    assert!(matches!(
        a.issue_validity(&delegated).await,
        Err(PortableJoinError::Revoked)
    ));
    assert!(
        c.import_revocation(&b_offer, &revoke, &receipt)
            .await
            .unwrap()
    );
    let c = PortableJoin::new(config.clone(), c_db.clone(), child.clone());
    let mut tx = c_db.begin().await.unwrap();
    assert!(matches!(
        c.authorize_local(&mut tx).await,
        Err(PortableJoinError::Revoked)
    ));
    tx.rollback().await.unwrap();

    let alternative = a
        .sign_offer(request(id.clone(), alternative, 10, false), window, 16384)
        .await
        .unwrap();
    a.approve(&id, &alternative, "operator", window, 16384)
        .await
        .unwrap();
    let alternative_lease = a.issue_validity(&alternative).await.unwrap();
    assert!(
        !c.import_chain(
            child.node_id(),
            &child.public_key_hex(),
            &alternative,
            &alternative_lease
        )
        .await
        .unwrap()
    );
    let mut tx = c_db.begin().await.unwrap();
    assert_eq!(
        c.authorize_local(&mut tx).await.unwrap(),
        Some(alternative_lease.expires_at)
    );
    tx.rollback().await.unwrap();
    assert!(
        b.import_revocation(&b_offer, &revoke, &receipt)
            .await
            .unwrap()
    );
    let mut tx = b_db.begin().await.unwrap();
    assert!(matches!(
        b.authorize_local(&mut tx).await,
        Err(PortableJoinError::Revoked)
    ));
    tx.rollback().await.unwrap();

    // Um recibo atesta o tempo da cadeia. O receptor ignora pisos de novas
    // decisões e aceita o bloqueio mesmo após a janela da folha.
    let late_db = isolated_pool("late").await.unwrap();
    let late = PortableJoin::new(config, late_db.clone(), child.clone());
    sqlx::query("INSERT INTO federation_portable_roots VALUES ($1, $2, $3)")
        .bind(late.cluster())
        .bind(root.node_id())
        .bind(root.public_key_hex())
        .execute(&late_db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO federation_portable_epoch VALUES ($1, $2, $3, 9999)")
        .bind(late.cluster())
        .bind(root.node_id())
        .bind(signer.node_id())
        .execute(&late_db)
        .await
        .unwrap();
    assert!(
        late.import_revocation(&sibling, &child_revoke, &child_receipt)
            .await
            .unwrap()
    );
    let mut too_old: JoinValidityV1 = alternative_lease;
    too_old.issued_at -= 1;
    too_old.sign(&root);
    assert!(matches!(
        c.import_chain(
            child.node_id(),
            &child.public_key_hex(),
            &alternative,
            &too_old
        )
        .await,
        Err(PortableJoinError::Validity)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_recupera_confirmacao_apos_timeout_e_recibo_apos_expiracao() {
    use std::time::Duration;

    use godmesh_federation::join_authority::JoinRevocationV1;
    use godmesh_federation::join_chain::JoinChainV2;
    use godmesh_federation::now_unix;
    use godmesh_server::admission::AdmissionConfig;
    use serde_json::{Value, json};

    async fn post(addr: std::net::SocketAddr, path: String, body: Value) -> common::HttpResponse {
        tokio::task::spawn_blocking(move || {
            common::call(
                addr,
                "POST",
                &path,
                Some("admin-test"),
                Some(&body.to_string()),
            )
        })
        .await
        .unwrap()
    }

    let Some(a_db) = isolated_pool("http_root").await else {
        return;
    };
    let b_db = isolated_pool("http_receiver").await.unwrap();
    let root = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let member = Arc::new(NodeIdentity::generate_in_memory().unwrap());
    let env = configuration(&root);
    let cfg = PortableJoinConfig::from_env_with(|key| env.get(key).cloned())
        .unwrap()
        .unwrap();
    let a = Arc::new(PortableJoin::new(cfg.clone(), a_db.clone(), root.clone()));
    let b = Arc::new(PortableJoin::new(cfg, b_db, member));
    let (root_addr, root_server) = serve_portable(
        a.clone(),
        AdmissionConfig {
            request_timeout: Duration::from_millis(250),
            ..AdmissionConfig::default()
        },
    )
    .await;
    let (peer_addr, peer_server) = serve_portable(b.clone(), AdmissionConfig::default()).await;

    let get = |addr, path: String| {
        tokio::task::spawn_blocking(move || common::get(addr, &path, "admin-test"))
    };
    let manifest = get(peer_addr, "/v1/federation/local-manifest".into())
        .await
        .unwrap()
        .json()["manifest"]
        .clone();
    let reply = post(
        root_addr,
        "/v1/federation/join-requests".into(),
        manifest.clone(),
    )
    .await;
    assert_eq!(reply.status, 201, "{}", reply.body);
    let request = reply.json();
    let id = request["join_request_id"].as_str().unwrap();
    let legacy = post(
        root_addr,
        format!("/v1/federation/join-requests/{id}/approve"),
        json!({}),
    )
    .await;
    assert_eq!(legacy.json()["error"], "federation_governance_disabled");
    let now = now_unix();
    let expiry = now + 8;
    let offer = post(
        root_addr,
        "/v1/federation/join-signatures".into(),
        json!({
            "join_request_id": id, "manifest": manifest, "can_approve": false,
            "not_before": now, "expires_at": expiry, "epoch": 1,
        }),
    )
    .await;
    assert_eq!(offer.status, 200, "{}", offer.body);
    let offer = offer.json();
    let approval_path = format!("/v1/federation/join-requests/{id}/approve-v2");
    let proof_path = format!("/v1/federation/join-requests/{id}/proof");

    let mut lock = a_db.begin().await.unwrap();
    a.lock_cluster(&mut lock).await.unwrap();
    let timed_out = post(root_addr, approval_path.clone(), offer.clone()).await;
    assert_eq!(timed_out.status, 503, "{}", timed_out.body);
    assert_eq!(timed_out.json()["error"], "request_timeout");
    assert_eq!(timed_out.json()["effect"], "unknown");
    lock.rollback().await.unwrap();
    assert_eq!(
        get(root_addr, proof_path.clone()).await.unwrap().status,
        404
    );
    let confirmed = post(root_addr, approval_path.clone(), offer.clone()).await;
    assert_eq!(confirmed.status, 201, "{}", confirmed.body);
    assert_eq!(confirmed.json(), offer);
    assert_eq!(get(root_addr, proof_path).await.unwrap().json(), offer);

    let issued = post(
        root_addr,
        "/v1/federation/join-validity".into(),
        offer.clone(),
    )
    .await;
    assert_eq!(issued.status, 200, "{}", issued.body);
    assert_eq!(issued.json()["validity"]["expires_at"], expiry);
    let chain: JoinChainV2 = serde_json::from_str(offer["chain_json"].as_str().unwrap()).unwrap();
    let mut revoke = JoinRevocationV1 {
        cluster: a.cluster().into(),
        root_node_id: root.node_id().into(),
        target_digest: chain.links[0].certificate.digest().unwrap(),
        revoker_node_id: String::new(),
        epoch: 1,
        signature: String::new(),
    };
    revoke.sign(&root);
    let declaration = json!({"target_chain_json": offer["chain_json"], "revocation": revoke});
    let revoked = post(
        root_addr,
        "/v1/federation/join-revocations".into(),
        declaration.clone(),
    )
    .await;
    assert_eq!(revoked.status, 201, "{}", revoked.body);
    let receipt = revoked.json();
    assert_eq!(
        post(
            root_addr,
            "/v1/federation/join-validity".into(),
            offer.clone()
        )
        .await
        .json()["error"],
        "join_proof_revoked"
    );

    tokio::time::sleep(Duration::from_secs((expiry - now_unix()).max(0) as u64)).await;
    assert!(now_unix() >= expiry);
    let replay = post(root_addr, approval_path, offer.clone()).await;
    assert_eq!(replay.status, 200, "{}", replay.body);
    assert_eq!(replay.json(), offer);
    let replay = post(
        root_addr,
        "/v1/federation/join-revocations".into(),
        declaration,
    )
    .await;
    assert_eq!(replay.status, 200, "{}", replay.body);
    assert_eq!(replay.json(), receipt);
    let late = post(peer_addr, "/v1/federation/join-revocation-imports".into(), json!({
        "target_chain_json": offer["chain_json"], "revocation": receipt["revocation"], "receipt": receipt["receipt"],
    })).await;
    assert_eq!(late.status, 201, "{}", late.body);

    root_server.abort();
    peer_server.abort();
}
