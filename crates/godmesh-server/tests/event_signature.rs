//! S-sec-2 — assinatura de nó no ingest de evento (`POST /v1/events`).
//!
//! A regra dura testada aqui, de
//! `docs/04-seguranca/abuso-e-malha-maligna.md` §5: **evento sem assinatura de
//! nó autorizado não entra no log durável.** Cada cenário negativo confere a
//! recusa no fio (HTTP/1.1 cru, como no resto da suíte) **e** a ausência de
//! efeito por SQL antes/depois — recusa com linha escrita é um 422 mentiroso.
//!
//! Banco próprio desta suíte, esquema aplicado pelo runner oficial via
//! `tests/common`. Sem `DATABASE_URL` os testes são pulados, não fingem verde.
//!
//! Cenários:
//!   1. nó `authorized` + escopo → 201 (e replay do MESMO envelope → 200);
//!   2. assinatura inválida → 422 `event_signature_invalid`, 0 linhas;
//!   3. nó `pending` → 422 `event_node_unauthorized`, 0 linhas;
//!   4. nó `authorized` sem escopo para o tenant/stream → 422
//!      `event_scope_denied`, 0 linhas;
//!   5. `authority_epoch` regressivo → 422 `event_epoch_stale`, nada novo;
//!   6. `issued_at` velho (ou do futuro) → 422 `event_not_fresh`, 0 linhas;
//!   7. origem forjada (nó X assina com origem = nó Y) → 422
//!      `event_scope_denied`;
//!   8. tenant assinado ≠ tenant do grant → 422 `event_scope_denied`;
//!   9. corpo sem assinatura (o formato antigo) → 422 `invalid_request`:
//!      não existe caminho "sem assinatura", nem por flag.

mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};

use godmesh_federation::NodeIdentity;
use godmesh_federation::event::EventDraft;
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{GrantRegistry, Scope, Subject, VerifiedGrant};
use serde_json::Value;
use sqlx::PgPool;

const TENANT: &str = "tenant-evt-sig-br";
const OTHER_TENANT: &str = "tenant-evt-sig-outro";
const TOKEN: &str = "grant-evt-sig";
const CLASS: &str = "mensagem";

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{tag}-{}-{n}", std::process::id())
}

fn grant(tenant: &str) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant.to_owned(),
            principal: "principal-evt-sig".to_owned(),
        },
        predicate: GrantPredicate {
            operation: OfflineOp::RecordLocalObservation,
            epoch_comparable: true,
            revocation: RevocationState::Absent,
            freshness: FreshnessSource::F3VerifiedOnline,
            retention_class_known: true,
            action_enumerated: true,
        },
        token_id: "test-grant".to_owned(),
        scopes: Scope::ALL.to_vec(),
        authority_epoch: 1,
    }
}

async fn pool() -> Option<PgPool> {
    common::pool_com_conexoes(4).await
}

async fn declare_tenant(pool: &PgPool, tenant: &str) {
    sqlx::query(
        "INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, 'BR') \
         ON CONFLICT (tenant_id) DO NOTHING",
    )
    .bind(tenant)
    .execute(pool)
    .await
    .expect("jurisdição declarada");
    sqlx::query(
        "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
         VALUES ('mensagem', 'BR', 'historico', 24, 'expiry') ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .expect("classe mensagem presente");
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         VALUES ('mensagem', 'BR', 'primary') ON CONFLICT DO NOTHING",
    )
    .execute(pool)
    .await
    .expect("cópia primária presente");
}

async fn no_autorizado(pool: &PgPool, stream: &str) -> NodeIdentity {
    let node = NodeIdentity::generate_in_memory().expect("identidade de teste");
    common::registrar_no(pool, &node, "authorized").await;
    common::conceder_escopo(pool, node.node_id(), TENANT, stream).await;
    node
}

async fn serve(pool: PgPool) -> SocketAddr {
    let grants =
        GrantRegistry::from_entries(vec![(TOKEN.to_owned(), grant(TENANT))]);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = godmesh_server::http::router(pool, grants);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn post_event(addr: SocketAddr, body: &str) -> (u16, Value) {
    let response = common::call(addr, "POST", "/v1/events", Some(TOKEN), Some(body));
    (response.status, response.json())
}

/// Contagem de `event_position` por stream: a prova de que recusa não grava.
async fn rows(pool: &PgPool, stream: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM event_position WHERE tenant_id = $1 AND stream_id = $2",
    )
    .bind(TENANT)
    .bind(stream)
    .fetch_one(pool)
    .await
    .expect("contagem")
}

async fn insert_reservation(connection: &mut sqlx::PgConnection, origin: &str, stream: &str) {
    sqlx::query(
        "INSERT INTO federation_replica_namespace (
             origin_node, tenant_id, stream_id, partition, authority_epoch,
             local_node, profile, profile_version, retention_class, class_version,
             jurisdiction, policy_id, policy_version
         ) VALUES ($1, $2, $3, 0, 7, $1, 'metadata', 1,
                   'm4a_metadados_posicao', 1, 'BR', 'fixture', 1)",
    )
    .bind(origin)
    .bind(TENANT)
    .bind(stream)
    .execute(connection)
    .await
    .expect("reserva sintética");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn escritores_legados_recusam_namespace_reservado_em_qualquer_epoch() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let mut outcomes = Vec::new();
    for epoch in [7, 9] {
        for inbox in [false, true] {
            let stream = unique("m4a.metadata.legado");
            let node = no_autorizado(&pool, &stream).await;
            insert_reservation(&mut pool.acquire().await.unwrap(), node.node_id(), &stream).await;
            let (route, body) = if inbox {
                let positions: Vec<_> = [1, 2]
                    .iter()
                    .map(|seq| {
                        serde_json::json!({
                            "origin_node": node.node_id(), "stream_id": stream,
                            "partition": 0, "authority_epoch": epoch, "seq": seq,
                        })
                    })
                    .collect();
                (
                    "/v1/sync/inbox",
                    serde_json::json!({"retention_class": CLASS, "positions": positions}).to_string(),
                )
            } else {
                (
                    "/v1/events",
                    common::envelope_assinado(&node, TENANT, &stream, 0, epoch, 1, CLASS),
                )
            };
            let response = tokio::task::spawn_blocking(move || {
                common::call(addr, "POST", route, Some(TOKEN), Some(&body))
            })
            .await
            .unwrap();
            let checkpoints: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM sync_inbox_state WHERE tenant_id = $1 AND stream_id = $2",
            )
            .bind(TENANT)
            .bind(&stream)
            .fetch_one(&pool)
            .await
            .unwrap();
            let error = response.json();
            outcomes.push((
                route,
                epoch,
                response.status,
                error["error"].clone(),
                rows(&pool, &stream).await,
                checkpoints,
            ));
        }
    }
    assert!(
        outcomes.iter().all(|(_, _, status, code, positions, checkpoints)| {
            *status == 409
                && *code == "event_namespace_reserved"
                && *positions == 0
                && *checkpoints == 0
        }),
        "recusa sem posição nem checkpoint em cada rota e epoch: {outcomes:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ingest_observa_reserva_confirmada_enquanto_aguarda_o_namespace() {
    let Some(pool) = pool().await else { return };
    let url = common::database_url().expect("URL do banco de teste");
    let writer_pool = common::pool_para(&url, 1).await;
    let writer: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&writer_pool)
        .await
        .unwrap();
    let addr = serve(writer_pool).await;
    declare_tenant(&pool, TENANT).await;
    let stream = unique("m4a.metadata.reserva-primeiro");
    let node = no_autorizado(&pool, &stream).await;
    let mut reservation = pool.begin().await.unwrap();
    let holder: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *reservation)
        .await
        .unwrap();
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended(
            jsonb_build_array('godmesh.event_position'::text, $1::text, $2::text,
                              $3::text, 0::int)::text, 0))",
    )
    .bind(node.node_id())
    .bind(TENANT)
    .bind(&stream)
    .execute(&mut *reservation)
    .await
    .unwrap();
    insert_reservation(&mut reservation, node.node_id(), &stream).await;
    let body = common::envelope_assinado(&node, TENANT, &stream, 0, 9, 1, CLASS);
    let pending = tokio::task::spawn_blocking(move || post_event(addr, &body));
    aguardar_bloqueio(&pool, writer, holder, "advisory").await;
    reservation.commit().await.unwrap();

    let (status, error) = pending.await.unwrap();
    assert_eq!(status, 409, "{error}");
    assert_eq!(error["error"], "event_namespace_reserved");
    assert_eq!(rows(&pool, &stream).await, 0);
}

/// 1 + replay — caminho feliz idempotente, com contagem antes/depois.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn evento_assinado_por_no_autorizado_e_201_e_replay_e_200_sem_duplicar() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-ok");
    let node = no_autorizado(&pool, &stream).await;

    assert_eq!(rows(&pool, &stream).await, 0, "antes: nada durável");
    let body =
        common::envelope_assinado(&node, TENANT, &stream, 1, 7, 1, CLASS);

    let (status, ack) = tokio::task::spawn_blocking({
        let body = body.clone();
        move || post_event(addr, &body)
    })
    .await
    .expect("cliente HTTP");
    assert_eq!(status, 201, "envelope válido precisa ser 201: {ack}");
    assert_eq!(ack["ack_class"], "persisted_local");
    assert_eq!(ack["seq"], 1);
    assert_eq!(rows(&pool, &stream).await, 1, "depois: exatamente uma linha");

    // O MESMO envelope assinado (mesmo issued_at, mesma assinatura) é replay
    // idempotente: 200, mesma posição, nenhuma segunda linha.
    let (status, replay) = tokio::task::spawn_blocking(move || post_event(addr, &body))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 200, "replay precisa ser 200: {replay}");
    assert_eq!(replay["seq"], 1);
    assert_eq!(replay["authority_epoch"], 7);
    assert_eq!(rows(&pool, &stream).await, 1, "replay não duplica");
}

/// 2 — assinatura que não confere: 422 nomeado, zero linhas.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn assinatura_invalida_e_422_event_signature_invalid_e_nada_e_gravado() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-assinatura");
    let node = no_autorizado(&pool, &stream).await;

    let mut body: Value = serde_json::from_str(&common::envelope_assinado(
        &node,
        TENANT,
        &stream,
        1,
        7,
        1,
        CLASS,
    ))
    .expect("envelope de teste serializa");
    let signature = body["signature"].as_str().expect("assinatura presente").to_owned();
    let mut flipped = signature.chars().collect::<Vec<_>>();
    let last = flipped.len() - 1;
    flipped[last] = if flipped[last] == 'a' { 'b' } else { 'a' };
    body["signature"] = Value::String(flipped.into_iter().collect());

    let before = rows(&pool, &stream).await;
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &body.to_string()))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "assinatura inválida precisa ser 422: {error}");
    assert_eq!(error["error"], "event_signature_invalid", "{error}");
    assert_eq!(rows(&pool, &stream).await, before, "422 não pode gravar");
}

/// 3 — nó **pendente** (manifesto ok, aprovação não): assinatura confere, mas
/// o registro não é autorizado; a recusa nomeia o nó, e nada é durável.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_pendente_e_422_event_node_unauthorized_e_nada_e_gravado() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-pendente");

    let pending = NodeIdentity::generate_in_memory().expect("identidade pendente");
    common::registrar_no(&pool, &pending, "pending").await;
    common::conceder_escopo(&pool, pending.node_id(), TENANT, &stream).await;

    let before = rows(&pool, &stream).await;
    let body = common::envelope_assinado(&pending, TENANT, &stream, 1, 7, 1, CLASS);
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &body))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "nó pendente não escreve: {error}");
    assert_eq!(error["error"], "event_node_unauthorized", "{error}");
    assert_eq!(rows(&pool, &stream).await, before, "422 não pode gravar");
}

/// Nó inexistente no registro também é `event_node_unauthorized`: sem chave
/// registrada não há o que verificar — e o erro diz isso, não "sem assinatura".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_sem_registro_e_422_event_node_unauthorized() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-sem-registro");

    let orphan = NodeIdentity::generate_in_memory().expect("identidade órfã");
    // Assinatura autoconsistente, mas o nó não está em `federation_peer`.
    let body = common::envelope_assinado(&orphan, TENANT, &stream, 1, 7, 1, CLASS);
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &body))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "{error}");
    assert_eq!(error["error"], "event_node_unauthorized", "{error}");
    assert_eq!(rows(&pool, &stream).await, 0);
}

/// 4 — nó `authorized` **sem escopo** para o tenant/stream: a assinatura é
/// válida, o nó é confiável, e ainda assim a resposta é 422 `event_scope_denied`
/// com zero linhas. Escopo ausente é negativa, não default.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_autorizado_sem_escopo_e_422_event_scope_denied_e_nada_e_gravado() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-sem-escopo");

    let node = NodeIdentity::generate_in_memory().expect("identidade");
    common::registrar_no(&pool, &node, "authorized").await;
    // Deliberadamente sem `federation_peer_scope`.

    let before = rows(&pool, &stream).await;
    let body = common::envelope_assinado(&node, TENANT, &stream, 1, 7, 1, CLASS);
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &body))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "sem escopo não escreve: {error}");
    assert_eq!(error["error"], "event_scope_denied", "{error}");
    assert_eq!(rows(&pool, &stream).await, before, "422 não pode gravar");
}

/// 5 — epoch regressivo: depois de reconhecer a autoridade 7, um envelope
/// **validamente assinado** sob autoridade 5 não reabre histórico.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authority_epoch_regressivo_e_422_event_epoch_stale() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-epoch");
    let node = no_autorizado(&pool, &stream).await;

    let first =
        common::envelope_assinado(&node, TENANT, &stream, 1, 7, 1, CLASS);
    let (status, body) = tokio::task::spawn_blocking(move || post_event(addr, &first))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 201, "preparação epoch 7: {body}");

    let before = rows(&pool, &stream).await;
    let stale =
        common::envelope_assinado(&node, TENANT, &stream, 1, 5, 1, CLASS);
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &stale))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "epoch 5 depois de 7 precisa ser recusado: {error}");
    assert_eq!(error["error"], "event_epoch_stale", "{error}");
    assert_eq!(error["current"], 7, "a resposta nomeia a autoridade atual: {error}");
    assert_eq!(error["got"], 5, "e o epoch reivindicado: {error}");
    assert_eq!(rows(&pool, &stream).await, before, "422 não pode gravar");

    // Controle: epoch maior que o vigente é aceito (autoridade nova manda).
    let newer =
        common::envelope_assinado(&node, TENANT, &stream, 1, 9, 1, CLASS);
    let (status, ok) = tokio::task::spawn_blocking(move || post_event(addr, &newer))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 201, "epoch 9 depois de 7 é a autoridade nova: {ok}");
    assert_eq!(rows(&pool, &stream).await, before + 1);
}

/// 6 — `issued_at` fora da janela (velho e futuro): 422 `event_not_fresh`,
/// nada durável. A janela default é de 5 minutos
/// (`GODMESH_EVENT_MAX_AGE_SECONDS`); um envelope de 1 h atrás é o caso real
/// de envelope roubado e reenviado.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn issued_at_fora_da_janela_e_422_event_not_fresh() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-janela");
    let node = no_autorizado(&pool, &stream).await;

    let before = rows(&pool, &stream).await;
    let now = godmesh_federation::now_unix();

    let old = common::envelope(
        &node,
        EventDraft {
            tenant_id: TENANT,
            stream_id: &stream,
            partition: 1,
            authority_epoch: 7,
            seq: 1,
            retention_class: CLASS,
            issued_at: now - 3_600,
        },
    );
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &old))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "envelope de 1 h atrás precisa ser recusado: {error}");
    assert_eq!(error["error"], "event_not_fresh", "{error}");

    let future = common::envelope(
        &node,
        EventDraft {
            tenant_id: TENANT,
            stream_id: &stream,
            partition: 1,
            authority_epoch: 7,
            seq: 1,
            retention_class: CLASS,
            issued_at: now + 3_600,
        },
    );
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &future))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "envelope do futuro também: {error}");
    assert_eq!(error["error"], "event_not_fresh", "{error}");

    assert_eq!(rows(&pool, &stream).await, before, "422 não pode gravar");
}

/// 7 — o buraco que a fatia fecha: nó **autorizado** comprometido tentando
/// injetar evento **em nome de outro nó**. A assinatura até confere (foi feita
/// pela chave do agressor, que cobre a origem falsa), e mesmo assim a recusa é
/// `event_scope_denied`: origem da posição é o assinante, ou nada.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_autorizado_nao_pode_injetar_em_nome_de_outro_no() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-forjado");

    let agressor = NodeIdentity::generate_in_memory().expect("agressor");
    let vitima = NodeIdentity::generate_in_memory().expect("vítima");
    common::registrar_no(&pool, &agressor, "authorized").await;
    common::registrar_no(&pool, &vitima, "authorized").await;
    common::conceder_escopo(&pool, agressor.node_id(), TENANT, &stream).await;

    // O agressor assina um envelope cuja ORIGEM é a vítima — assinatura válida
    // do agressor sobre a origem falsa. Não passa: origem ≠ assinante.
    let claim = godmesh_federation::event::EventPositionClaim {
        origin_node: vitima.node_id().to_owned(),
        stream_id: stream.clone(),
        partition: 1,
        authority_epoch: 7,
        seq: 1,
    };
    let mut event = godmesh_federation::SignedEvent {
        position: claim,
        tenant_id: TENANT.to_owned(),
        retention_class: CLASS.to_owned(),
        issued_at: godmesh_federation::now_unix(),
        node_id: agressor.node_id().to_owned(),
        signature: String::new(),
    };
    event.signature = agressor.sign_hex(&event.canonical_bytes());
    let forged = serde_json::to_string(&event).expect("serializa forjado");

    let before = rows(&pool, &stream).await;
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &forged))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "origem forjada precisa ser recusada: {error}");
    assert_eq!(error["error"], "event_scope_denied", "{error}");
    assert_eq!(rows(&pool, &stream).await, before, "422 não pode gravar");
}

/// 8 — tenant assinado ≠ tenant autenticado: assinatura de nó autorizado não
/// amplia tenancy. (Um envelope assinado para OUTRO tenant teria também de
/// passar pelo escopo desse outro tenant; o portão do grant vem antes.)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tenant_assinado_diferente_do_grant_e_422_event_scope_denied() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-tenant");
    let node = no_autorizado(&pool, &stream).await;

    let body =
        common::envelope_assinado(&node, OTHER_TENANT, &stream, 1, 7, 1, CLASS);
    let before = rows(&pool, &stream).await;
    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &body))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "{error}");
    assert_eq!(error["error"], "event_scope_denied", "{error}");
    assert_eq!(rows(&pool, &stream).await, before, "422 não pode gravar");
}

/// 9 — **não existe caminho sem assinatura**: o corpo antigo (sem envelope) é
/// recusado na fronteira com `invalid_request`, e nenhuma linha aparece. A
/// rota não tem flag, default ou modo de transição que aceite o formato velho.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn corpo_sem_assinatura_e_recusado_na_fronteira_sem_efeito() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-sem-assinatura");

    let legado = serde_json::json!({
        "origin_node": "node-sb",
        "stream_id": stream,
        "partition": 1,
        "authority_epoch": 7,
        "seq": 1,
        "retention_class": CLASS,
    })
    .to_string();

    let (status, error) = tokio::task::spawn_blocking(move || post_event(addr, &legado))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 422, "formato antigo não tem mais porta: {error}");
    assert_eq!(error["error"], "invalid_request", "{error}");
    assert_eq!(rows(&pool, &stream).await, 0, "recusa na fronteira não grava");
}

/// Sincroniza pelo bloqueador observado no PostgreSQL. O prazo só limita
/// falhas; a ordem do teste não depende de atrasos entre os pedidos.
async fn aguardar_bloqueio(pool: &PgPool, waiter: i32, blocker: i32, lock_type: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT $2 = ANY(pg_blocking_pids($1)) AND EXISTS ( \
                     SELECT 1 FROM pg_locks WHERE pid = $1 AND locktype = $3 AND NOT granted)",
            )
            .bind(waiter)
            .bind(blocker)
            .bind(lock_type)
            .fetch_one(pool)
            .await
            .expect("estado dos locks");
            if waiting {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("esperava bloqueio {lock_type} de {waiter} por {blocker}"));
}

// Detém a escrita na política de retenção exclusiva do cenário, após o lock
// do namespace e, no ingest, após as leituras de autoridade e escopo.
async fn bloquear_retencao(
    pool: &PgPool,
    class: &str,
) -> (sqlx::Transaction<'static, sqlx::Postgres>, i32) {
    sqlx::query(
        "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
         VALUES ($1, 'BR', 'historico', 24, 'expiry')",
    )
    .bind(class)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) VALUES ($1, 'BR', 'primary')",
    )
    .bind(class)
    .execute(pool)
    .await
    .unwrap();

    let mut gate = pool.begin().await.unwrap();
    let pid = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *gate)
        .await
        .unwrap();
    sqlx::query(
        "SELECT class FROM retention_copy \
         WHERE class = $1 AND jurisdiction = 'BR' AND copy_scope = 'primary' FOR UPDATE",
    )
    .bind(class)
    .fetch_one(&mut *gate)
    .await
    .unwrap();
    (gate, pid)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ingest_e_inbox_serializam_a_primeira_escrita_em_ambas_as_ordens() {
    let Some(pool) = pool().await else { return };
    let url = common::database_url().expect("URL do banco de teste");
    let first_pool = common::pool_para(&url, 1).await;
    let second_pool = common::pool_para(&url, 1).await;
    let first_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&first_pool)
        .await
        .unwrap();
    let second_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&second_pool)
        .await
        .unwrap();
    let first_addr = serve(first_pool).await;
    let second_addr = serve(second_pool).await;
    declare_tenant(&pool, TENANT).await;

    for inbox_first in [false, true] {
        let stream = unique("stream-lock");
        let node = no_autorizado(&pool, &stream).await;
        assert_eq!(rows(&pool, &stream).await, 0);
        let (inbox_epoch, ingest_epoch) = if inbox_first { (7, 5) } else { (5, 7) };
        let event = common::envelope_assinado(&node, TENANT, &stream, 1, ingest_epoch, 1, &stream);
        let batch = serde_json::json!({
            "retention_class": stream,
            "positions": [{
                "origin_node": node.node_id(),
                "stream_id": stream,
                "partition": 1,
                "authority_epoch": inbox_epoch,
                "seq": 1
            }]
        })
        .to_string();

        let send = move |addr, inbox| {
            let (path, body) = if inbox {
                ("/v1/sync/inbox", batch.clone())
            } else {
                ("/v1/events", event.clone())
            };
            tokio::task::spawn_blocking(move || {
                let response = common::call(addr, "POST", path, Some(TOKEN), Some(&body));
                (response.status, response.json())
            })
        };

        let (gate, gate_pid) = bloquear_retencao(&pool, &stream).await;
        let first = send(first_addr, inbox_first);
        aguardar_bloqueio(&pool, first_pid, gate_pid, "transactionid").await;

        if !inbox_first {
            for query in [
                "SELECT node_id FROM federation_peer WHERE node_id = $1 FOR NO KEY UPDATE NOWAIT",
                "SELECT node_id FROM federation_peer_scope WHERE node_id = $1 FOR UPDATE NOWAIT",
            ] {
                let error = sqlx::query(query)
                    .bind(node.node_id())
                    .fetch_one(&pool)
                    .await
                    .expect_err("o ingest deve manter a linha protegida até o commit");
                assert_eq!(
                    error.as_database_error().unwrap().code().as_deref(),
                    Some("55P03")
                );
            }
        }

        let second = send(second_addr, !inbox_first);
        aguardar_bloqueio(&pool, second_pid, first_pid, "advisory").await;
        gate.commit().await.unwrap();

        let first = first.await.unwrap();
        let second = second.await.unwrap();
        let (inbox, ingest) = if inbox_first {
            (first, second)
        } else {
            (second, first)
        };
        assert_eq!(inbox.0, 200, "{}", inbox.1);
        assert_eq!(inbox.1["accepted"], 1);
        assert_eq!(inbox.1["contiguous_prefix"], 1);
        if inbox_first {
            assert_eq!(ingest.0, 422, "{}", ingest.1);
            assert_eq!(ingest.1["error"], "event_epoch_stale");
            assert_eq!(ingest.1["current"], 7);
            assert_eq!(ingest.1["got"], 5);
        } else {
            assert_eq!(ingest.0, 201, "{}", ingest.1);
        }

        let positions: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT authority_epoch, seq FROM event_position \
             WHERE tenant_id = $1 AND stream_id = $2 ORDER BY authority_epoch, seq",
        )
        .bind(TENANT)
        .bind(&stream)
        .fetch_all(&pool)
        .await
        .unwrap();
        let expected = if inbox_first {
            vec![(7, 1)]
        } else {
            vec![(5, 1), (7, 1)]
        };
        assert_eq!(positions, expected);

        let checkpoints: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT authority_epoch, contiguous_prefix FROM sync_inbox_state \
             WHERE tenant_id = $1 AND stream_id = $2",
        )
        .bind(TENANT)
        .bind(&stream)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(checkpoints, [(inbox_epoch, 1)]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ingest_cancelado_no_banco_libera_o_namespace_para_nova_tentativa() {
    let Some(pool) = pool().await else { return };
    let url = common::database_url().expect("URL do banco de teste");
    let writer = common::pool_para(&url, 1).await;
    let writer_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&writer)
        .await
        .unwrap();
    let addr = serve(writer).await;
    let stream = unique("stream-retry");
    declare_tenant(&pool, TENANT).await;
    let node = no_autorizado(&pool, &stream).await;
    let body = common::envelope_assinado(&node, TENANT, &stream, 1, 7, 1, &stream);

    let (gate, gate_pid) = bloquear_retencao(&pool, &stream).await;
    let request = tokio::task::spawn_blocking({
        let body = body.clone();
        move || post_event(addr, &body)
    });
    aguardar_bloqueio(&pool, writer_pid, gate_pid, "transactionid").await;
    let cancelled: bool = sqlx::query_scalar("SELECT pg_cancel_backend($1)")
        .bind(writer_pid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(cancelled);
    let (status, error) = request.await.unwrap();
    assert_eq!(status, 503, "{error}");
    assert_eq!(error["error"], "storage_unavailable");
    gate.rollback().await.unwrap();
    assert_eq!(rows(&pool, &stream).await, 0);

    // Outro pool comprova a liberação do lock entre sessões.
    let retry_addr = serve(common::pool_para(&url, 1).await).await;
    let (status, ack) = tokio::task::spawn_blocking(move || post_event(retry_addr, &body))
        .await
        .unwrap();
    assert_eq!(status, 201, "{ack}");
    assert_eq!(rows(&pool, &stream).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replay_observa_remocao_de_escopo_e_revogacao_ja_confirmadas() {
    let Some(pool) = pool().await else { return };
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-replay-policy");
    declare_tenant(&pool, TENANT).await;
    let node = no_autorizado(&pool, &stream).await;
    let send = |class: &str| {
        let body = common::envelope_assinado(&node, TENANT, &stream, 1, 7, 1, class);
        tokio::task::spawn_blocking(move || post_event(addr, &body))
    };

    let (status, ack) = send(CLASS).await.unwrap();
    assert_eq!(status, 201, "{ack}");
    sqlx::query(
        "DELETE FROM federation_peer_scope WHERE node_id = $1 AND tenant_id = $2 AND stream_id = $3",
    )
    .bind(node.node_id())
    .bind(TENANT)
    .bind(&stream)
    .execute(&pool)
    .await
    .unwrap();
    let (status, error) = send("posicao").await.unwrap();
    assert_eq!(status, 422, "{error}");
    assert_eq!(error["error"], "event_scope_denied");
    assert_eq!(rows(&pool, &stream).await, 1);

    common::conceder_escopo(&pool, node.node_id(), TENANT, &stream).await;
    let (status, replay) = send("posicao").await.unwrap();
    assert_eq!(status, 200, "{replay}");
    assert_eq!(replay, ack);
    common::registrar_no(&pool, &node, "revoked").await;
    let (status, error) = send("posicao").await.unwrap();
    assert_eq!(status, 422, "{error}");
    assert_eq!(error["error"], "event_node_unauthorized");
    assert_eq!(rows(&pool, &stream).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ingest_aceita_reenvio_depois_de_preencher_lacuna() {
    let Some(pool) = pool().await else { return };
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-gap-retry");
    declare_tenant(&pool, TENANT).await;
    let node = no_autorizado(&pool, &stream).await;
    let first = common::envelope_assinado(&node, TENANT, &stream, 1, 7, 1, CLASS);
    let next = common::envelope_assinado(&node, TENANT, &stream, 1, 7, 2, CLASS);
    let send = |body: String| tokio::task::spawn_blocking(move || post_event(addr, &body));

    let (status, gap) = send(next.clone()).await.unwrap();
    assert_eq!(status, 409, "{gap}");
    assert_eq!(gap["error"], "sequence_gap");
    let detail = gap["detail"].as_str().expect("detalhe da lacuna");
    assert!(detail.contains("posição 2"), "{detail}");
    assert!(detail.contains("espera 1"), "{detail}");
    assert_eq!(rows(&pool, &stream).await, 0);

    let (status, ack) = send(first).await.unwrap();
    assert_eq!(status, 201, "{ack}");
    let (status, ack) = send(next).await.unwrap();
    assert_eq!(status, 201, "{ack}");
    assert_eq!(ack["seq"], 2);
    assert_eq!(rows(&pool, &stream).await, 2);
}
