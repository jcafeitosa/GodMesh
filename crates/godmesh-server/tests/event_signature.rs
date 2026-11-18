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
         ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction",
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

/// 1 + replay — caminho feliz idempotente, com contagem antes/depois.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn evento_assinado_por_no_autorizado_e_201_e_replay_e_200_sem_duplicar() {
    let Some(pool) = pool().await else { return };
    declare_tenant(&pool, TENANT).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream-ok");
    common::no_local_pronto(&pool, TENANT, &stream).await;

    assert_eq!(rows(&pool, &stream).await, 0, "antes: nada durável");
    let body =
        common::envelope_assinado(&common::LOCAL_NODE, TENANT, &stream, 1, 7, 1, CLASS);

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
    common::no_local_pronto(&pool, TENANT, &stream).await;

    let mut body: Value = serde_json::from_str(&common::envelope_assinado(
        &common::LOCAL_NODE,
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
    common::no_local_pronto(&pool, TENANT, &stream).await;

    let first =
        common::envelope_assinado(&common::LOCAL_NODE, TENANT, &stream, 1, 7, 1, CLASS);
    let (status, body) = tokio::task::spawn_blocking(move || post_event(addr, &first))
        .await
        .expect("cliente HTTP");
    assert_eq!(status, 201, "preparação epoch 7: {body}");

    let before = rows(&pool, &stream).await;
    let stale =
        common::envelope_assinado(&common::LOCAL_NODE, TENANT, &stream, 1, 5, 1, CLASS);
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
        common::envelope_assinado(&common::LOCAL_NODE, TENANT, &stream, 1, 9, 1, CLASS);
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
    common::no_local_pronto(&pool, TENANT, &stream).await;

    let before = rows(&pool, &stream).await;
    let now = godmesh_federation::now_unix();

    let old = common::envelope(
        &common::LOCAL_NODE,
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
        &common::LOCAL_NODE,
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
    common::no_local_pronto(&pool, TENANT, &stream).await;

    let body =
        common::envelope_assinado(&common::LOCAL_NODE, OTHER_TENANT, &stream, 1, 7, 1, CLASS);
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
