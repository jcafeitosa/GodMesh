//! Suíte **end-to-end** HTTP do Programa A: as 21 verificações do roteiro manual
//! de 18/11/2026 (`docs/09-equipe/evidencias/2026-11-18-e2e-http.md`) como
//! asserções executáveis, para virar portão de CI em vez de roteiro de `curl`.
//!
//! Diferenças em relação ao roteiro manual, todas deliberadas:
//!  * **sem `curl`**: o cliente é HTTP/1.1 cru sobre `std::net::TcpStream`, o
//!    mesmo padrão de `tests/authorization.rs` e `tests/events.rs`. Nenhuma
//!    dependência nova entra no workspace por causa de teste;
//!  * **banco descartável**: `godmesh_e2e_<pid>`, criado do zero e migrado pelo
//!    *runner oficial* (`godmesh_server::migrate::migrate`, via
//!    `tests/common/mod.rs`). O banco de desenvolvimento (`godmesh`) não é
//!    tocado;
//!  * **router real em porta efêmera** com `GrantRegistry::from_entries`
//!    (composição de teste já existente); produção (`src/`) não foi alterada;
//!  * **efeito verificado no banco**: todo caminho negativo confere por SQL que
//!    nenhuma linha nova apareceu. Status HTTP correto com INSERT indevido
//!    ainda seria falha.
//!
//! Sem `GODMESH_TEST_DATABASE_URL`/`DATABASE_URL` os testes são pulados com
//! aviso explícito; com URL declarada e banco inacessível eles **falham**
//! (ambiente declarado e quebrado não pode parecer verde).
//!
//! Execução (ambiente local-first, ADR-070 — sem contêiner):
//!
//! ```text
//! DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh \
//!   cargo test -p godmesh-server --test e2e -- --nocapture
//! ```

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{GrantRegistry, Scope, Subject, VerifiedGrant};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};

/// Tenant A da suíte: o tenant "do pedido" em todos os caminhos felizes.
const TENANT_A: &str = "tenant-e2e-br-a";
/// Tenant B: existe, autentica, e mesmo assim não enxerga dado do tenant A.
const TENANT_B: &str = "tenant-e2e-br-b";

/// Token do tenant A (valor de teste; não é segredo real).
const TOKEN_A: &str = "grant-e2e-tenant-a";
/// Token do tenant B (valor de teste; não é segredo real).
const TOKEN_B: &str = "grant-e2e-tenant-b";

/// Token de outro tenant que **não** está no registro: é o caso do roteiro
/// manual em que a credencial inválida barra antes de qualquer avaliação de
/// tenancy.
const TOKEN_B_NAO_REGISTRADO: &str = "grant-e2e-tenant-b-fora-do-registro";
/// Token que não pertence a ninguém.
const TOKEN_DESCONHECIDO: &str = "token-que-nao-existe-em-lugar-nenhum";

/// Classe semeada pela migração 0001 para BR, com cópia primária.
const CLASS: &str = "mensagem";
/// Classe de retenção do incidente (`godmesh-emergency`), semeada pela 0001.
const CLASS_INCIDENTE: &str = "incidente_sos";

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Identificador único dentro da execução: os testes deste binário rodam em
/// paralelo e compartilham o mesmo banco descartável.
fn unique(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{tag}-{}-{n}", std::process::id())
}

// --- ambiente: banco descartável ---------------------------------------------

/// Ambiente da suíte. Só a URL: **nenhum pool é compartilhado entre testes**.
///
/// Um pool estático pertenceria ao runtime do primeiro teste que o criasse, e
/// os demais herdariam um pool cujo driver pode já ter sido derrubado. Cada
/// teste abre o seu e fecha ao sair.
struct Env {
    url: String,
}

static ENV: tokio::sync::OnceCell<Env> = tokio::sync::OnceCell::const_new();

/// Banco descartável do processo: `godmesh_e2e_<pid>`.
fn db_name() -> String {
    format!("godmesh_e2e_{}", std::process::id())
}

/// Troca só o nome do banco na URL (preserva credenciais, host, porta e query).
fn with_database(url: &str, db: &str) -> String {
    let (sem_query, query) = match url.split_once('?') {
        Some((a, b)) => (a, Some(b)),
        None => (url, None),
    };
    let corte = sem_query
        .rfind('/')
        .expect("DATABASE_URL precisa terminar em /banco");
    let raiz = &sem_query[..corte];
    match query {
        Some(q) => format!("{raiz}/{db}?{q}"),
        None => format!("{raiz}/{db}"),
    }
}

/// Cria o banco descartável do zero e aplica as migrações pelo runner oficial.
///
/// O `DROP … WITH (FORCE)` no início é o que torna o banco descartável de fato:
/// cada execução começa sem estado, e uma execução anterior interrompida não
/// contamina a próxima. É banco criado e usado só por este binário de teste.
async fn setup() -> Env {
    let base = common::database_url().expect("URL de banco presente (verificado antes)");
    let db = db_name();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_database(&base, "postgres"))
        .await
        .unwrap_or_else(|e| panic!("Postgres inacessível para administração ({base}): {e}"));

    sqlx::query(AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {db} WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .expect("drop do banco descartável");
    sqlx::query(AssertSqlSafe(format!("CREATE DATABASE {db}")))
        .execute(&admin)
        .await
        .expect("create do banco descartável");
    admin.close().await;

    let url = with_database(&base, &db);

    // Runner oficial (`--migrate-only` usa a mesma função): o teste exercita a
    // mesma ordem, o mesmo controle e o mesmo checksum da produção.
    let pool = common::pool_para(&url, 4).await;
    for tenant in [TENANT_A, TENANT_B] {
        sqlx::query(
            "INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, 'BR') \
             ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction",
        )
        .bind(tenant)
        .execute(&pool)
        .await
        .expect("jurisdição BR declarada para o tenant de teste");
    }
    // As classes usadas nos testes precisam de política e cópia primária na
    // jurisdição (ADR-062); a 0002 já declara `tenant_region`, mas a semeadura de
    // retenção da 0001/0002 é conferida aqui para falhar alto se sumir.
    for class in [CLASS, CLASS_INCIDENTE] {
        sqlx::query(
            "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
             SELECT $1, 'BR', 'historico', 12, 'expiry' WHERE NOT EXISTS (\
                 SELECT 1 FROM retention_class WHERE class = $1 AND jurisdiction = 'BR')",
        )
        .bind(class)
        .execute(&pool)
        .await
        .expect("classe de retenção presente");
        sqlx::query(
            "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
             SELECT $1, 'BR', 'primary' WHERE NOT EXISTS (\
                 SELECT 1 FROM retention_copy \
                  WHERE class = $1 AND jurisdiction = 'BR' AND copy_scope = 'primary')",
        )
        .bind(class)
        .execute(&pool)
        .await
        .expect("cópia primária presente");
    }
    pool.close().await;

    eprintln!("e2e: banco descartável pronto em {url}");
    Env { url }
}

/// Ambiente da suíte, ou `None` quando nenhuma URL de banco foi declarada.
async fn env() -> Option<&'static Env> {
    if common::database_url().is_none() {
        eprintln!(
            "SKIP: sem GODMESH_TEST_DATABASE_URL/DATABASE_URL; a suíte E2E depende de Postgres"
        );
        return None;
    }
    Some(ENV.get_or_init(setup).await)
}

/// Pool próprio do teste, no banco descartável já migrado.
async fn pool(env: &Env) -> PgPool {
    common::pool_para(&env.url, 4).await
}

// --- registro de grants e router ---------------------------------------------

fn predicate() -> GrantPredicate {
    GrantPredicate {
        operation: OfflineOp::OpenSosIncident,
        epoch_comparable: true,
        revocation: RevocationState::Absent,
        freshness: FreshnessSource::F3VerifiedOnline,
        retention_class_known: true,
        action_enumerated: true,
    }
}

fn grant(tenant: &str, token_id: &str) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant.to_owned(),
            principal: format!("principal-{tenant}"),
        },
        predicate: predicate(),
        token_id: token_id.to_owned(),
        // Escopo vazio em `from_entries` significa "todos" (composição de
        // teste); declarado explicitamente para não depender desse default.
        scopes: Scope::ALL.to_vec(),
        authority_epoch: 7,
    }
}

/// Dois grants: um por tenant. Prova que tenancy vem do grant, não do corpo.
fn registry() -> GrantRegistry {
    GrantRegistry::from_entries(vec![
        (TOKEN_A.to_owned(), grant(TENANT_A, "g-e2e-a")),
        (TOKEN_B.to_owned(), grant(TENANT_B, "g-e2e-b")),
    ])
}

/// Sobe o router **real** (composição de produção do `router`) em porta efêmera.
async fn serve(pool: PgPool) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let app = godmesh_server::http::router(pool, registry());
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

// --- cliente HTTP/1.1 sem dependência nova -----------------------------------

/// Resposta lida do fio: status, headers e corpo.
struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Response {
    /// Corpo JSON. Falha alto: corpo não-JSON é violação do contrato observável.
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| {
            panic!("corpo não é JSON ({e}): {:?} (status {})", self.body, self.status)
        })
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Uma requisição HTTP/1.1 crua: `Connection: close`, sem keep-alive, para que
/// ler até o fim do socket seja ler a resposta inteira.
fn call(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> Response {
    let mut stream = TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de escrita");

    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\n");
    if let Some(value) = token {
        request.push_str(&format!("Authorization: Bearer {value}\r\n"));
    }
    if let Some(body) = body {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("Connection: close\r\n\r\n");
    if let Some(body) = body {
        request.push_str(body);
    }

    stream.write_all(request.as_bytes()).expect("envia pedido");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("lê resposta");

    let (head, body) = raw
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("resposta HTTP sem cabeçalho: {raw:?}"));
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {raw:?}"));
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();

    Response {
        status,
        headers,
        body: body.to_owned(),
    }
}

fn post(addr: SocketAddr, path: &str, token: Option<&str>, body: &str) -> Response {
    call(addr, "POST", path, token, Some(body))
}

fn get(addr: SocketAddr, path: &str, token: Option<&str>) -> Response {
    call(addr, "GET", path, token, None)
}

fn incident_body(id: &str) -> String {
    json!({ "incident_id": id }).to_string()
}

fn event_body(stream: &str, seq: u64, class: &str) -> String {
    json!({
        "origin_node": "node-e2e",
        "stream_id": stream,
        "partition": 1,
        "authority_epoch": 7,
        "seq": seq,
        "retention_class": class,
    })
    .to_string()
}

// --- leitura do banco: o efeito, não a promessa -------------------------------

async fn incident_rows(pool: &PgPool, id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sos_incident WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("contagem de sos_incident")
}

async fn ack_rows(pool: &PgPool, id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM sos_ack WHERE incident_id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("contagem de sos_ack")
}

async fn incident_state(pool: &PgPool, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT state FROM sos_incident WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .expect("estado do incidente")
}

async fn event_rows(pool: &PgPool, tenant: &str, stream: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM event_position WHERE tenant_id = $1 AND stream_id = $2",
    )
    .bind(tenant)
    .bind(stream)
    .fetch_one(pool)
    .await
    .expect("contagem de event_position")
}

/// Leva um incidente de `persisted_local` a `closed` pelas rotas públicas.
/// Usado pelo teste de transição ilegal (verificação 20).
async fn levar_ate_closed(addr: SocketAddr, id: &str) {
    for (path, body) in [
        (format!("/v1/incidents/{id}/replicate"), "".to_owned()),
        (format!("/v1/incidents/{id}/ack-technical"), "".to_owned()),
        (
            format!("/v1/incidents/{id}/acknowledge"),
            json!({ "responder": "ana.souza" }).to_string(),
        ),
        (format!("/v1/incidents/{id}/close"), "".to_owned()),
    ] {
        let res = post(addr, &path, Some(TOKEN_A), &body);
        assert_eq!(res.status, 200, "transição {path} precisa ser 200: {}", res.body);
    }
}

// --- verificação 1 -----------------------------------------------------------

/// Verificação 1 do roteiro: sem credencial → 401 `unauthenticated`, e **nada**
/// é gravado (o handler não roda).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sem_credencial_e_401_e_nao_insere() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(pool.clone()).await;
    let id = unique("sem-credencial");

    let res = post(addr, "/v1/incidents", None, &incident_body(&id));
    assert_eq!(res.status, 401, "sem credencial precisa ser 401: {}", res.body);
    let body = res.json();
    assert_eq!(body["error"], "unauthenticated", "código estável: {body}");
    assert_eq!(body["ack_class"], "transport_accepted", "nada foi confirmado");
    assert_eq!(
        res.header("www-authenticate"),
        Some("Bearer"),
        "401 precisa anunciar o esquema"
    );
    assert!(
        !res.body.contains(&id),
        "a recusa não pode ecoar o pedido: {}",
        res.body
    );

    assert_eq!(incident_rows(&pool, &id).await, 0, "401 não pode inserir incidente");
    assert_eq!(ack_rows(&pool, &id).await, 0, "401 não pode inserir ACK");
    pool.close().await;
}

// --- verificações 2 a 8 -------------------------------------------------------

/// Verificações 2–8: criação 201, replay 200, `/replicate`, `/ack-technical`,
/// `/acknowledge` sem e com `responder`, e `/close` com trilha de 4 ACKs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incidente_do_persistido_ao_closed_com_trilha_de_quatro_acks() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(pool.clone()).await;
    let id = unique("ciclo");

    // 2. criação
    let res = post(addr, "/v1/incidents", Some(TOKEN_A), &incident_body(&id));
    assert_eq!(res.status, 201, "criação precisa ser 201: {}", res.body);
    let body = res.json();
    assert_eq!(body["incident_id"], id.as_str());
    assert_eq!(body["state"], "persisted_local");
    assert_eq!(body["ack_class"], "persisted_local");
    assert_eq!(body["ui_label"], "registrado localmente, não enviado");

    // 3. replay do mesmo incident_id: idempotente, sem duplicar linha.
    let res = post(addr, "/v1/incidents", Some(TOKEN_A), &incident_body(&id));
    assert_eq!(res.status, 200, "replay precisa ser 200: {}", res.body);
    assert_eq!(res.json()["state"], "persisted_local");
    assert_eq!(incident_rows(&pool, &id).await, 1, "replay não duplica incidente");
    assert_eq!(ack_rows(&pool, &id).await, 1, "replay não duplica ACK");

    // 4. replica
    let res = post(addr, &format!("/v1/incidents/{id}/replicate"), Some(TOKEN_A), "");
    assert_eq!(res.status, 200, "replicate precisa ser 200: {}", res.body);
    assert_eq!(res.json()["state"], "replicated_to_authority");

    // 5. reconhecimento técnico
    let res = post(
        addr,
        &format!("/v1/incidents/{id}/ack-technical"),
        Some(TOKEN_A),
        "",
    );
    assert_eq!(res.status, 200, "ack-technical precisa ser 200: {}", res.body);
    let body = res.json();
    assert_eq!(body["state"], "acknowledged_technical");
    assert_eq!(body["ack_class"], "materialized");
    assert_eq!(body["acks"].as_array().map(Vec::len), Some(3), "trilha: {body}");

    // 6. reconhecimento humano sem pessoa: 422 e estado intacto.
    let res = post(
        addr,
        &format!("/v1/incidents/{id}/acknowledge"),
        Some(TOKEN_A),
        "{}",
    );
    assert_eq!(res.status, 422, "sem responder precisa ser 422: {}", res.body);
    assert_eq!(res.json()["error"], "missing_human_proof");
    assert_eq!(
        incident_state(&pool, &id).await.as_deref(),
        Some("acknowledged_technical"),
        "recusa não pode mover o estado"
    );
    assert_eq!(ack_rows(&pool, &id).await, 3, "recusa não pode gravar ACK");

    // 7. reconhecimento humano com `responder`.
    let res = post(
        addr,
        &format!("/v1/incidents/{id}/acknowledge"),
        Some(TOKEN_A),
        &json!({ "responder": "ana.souza" }).to_string(),
    );
    assert_eq!(res.status, 200, "acknowledge com pessoa: {}", res.body);
    let body = res.json();
    assert_eq!(body["state"], "acknowledged_human");
    assert_eq!(body["ack_class"], "acknowledged_human");

    // 8. encerramento: estado `closed`, ACK humano mantido, trilha com 4 ACKs.
    let res = post(addr, &format!("/v1/incidents/{id}/close"), Some(TOKEN_A), "");
    assert_eq!(res.status, 200, "close precisa ser 200: {}", res.body);
    let body = res.json();
    assert_eq!(body["state"], "closed");
    assert_eq!(body["ack_class"], "acknowledged_human");
    assert_eq!(body["ui_label"], "encerrado");

    let acks = body["acks"].as_array().expect("trilha de ACKs").clone();
    assert_eq!(acks.len(), 4, "trilha precisa ter 4 ACKs: {body}");
    let mut classes: Vec<&str> = acks
        .iter()
        .map(|a| a["ack_class"].as_str().expect("ack_class na trilha"))
        .collect();
    classes.sort_unstable();
    assert_eq!(
        classes,
        vec![
            "acknowledged_human",
            "materialized",
            "persisted_local",
            "replicated_to_authority"
        ],
        "as quatro provas do ciclo: {body}"
    );
    for ack in &acks {
        let at_unix = ack["at_unix"].as_i64().expect("at_unix na trilha");
        assert!(at_unix > 0, "ACK sem instante não é evidência: {ack}");
        assert!(
            !ack["source"].as_str().unwrap_or_default().is_empty(),
            "ACK precisa de fonte: {ack}"
        );
    }
    assert_eq!(ack_rows(&pool, &id).await, 4, "banco guarda as 4 provas");
    assert_eq!(
        incident_state(&pool, &id).await.as_deref(),
        Some("closed"),
        "estado persistido é a fonte do rótulo"
    );
    pool.close().await;
}

// --- verificações 19 e 20 -----------------------------------------------------

/// Verificações 20 e 19: transição a partir de `closed` → 409 `illegal_transition`
/// com `state=closed` e nenhuma prova nova; incidente inexistente → 404
/// `incident_not_found` (o mesmo 404 de incidente de outro tenant).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transicao_apos_closed_e_409_e_incidente_inexistente_e_404() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(pool.clone()).await;
    let id = unique("ilegal");
    let inexistente = unique("inexistente");

    let res = post(addr, "/v1/incidents", Some(TOKEN_A), &incident_body(&id));
    assert_eq!(res.status, 201, "preparação: {}", res.body);
    levar_ate_closed(addr, &id).await;

    // 20. repetir `close` a partir de `closed`.
    let res = post(addr, &format!("/v1/incidents/{id}/close"), Some(TOKEN_A), "");
    assert_eq!(res.status, 409, "transição ilegal precisa ser 409: {}", res.body);
    let body = res.json();
    assert_eq!(body["error"], "illegal_transition", "código estável: {body}");
    assert_eq!(body["state"], "closed", "o corpo precisa nomear o estado atual");
    assert!(
        body["detail"].as_str().unwrap_or_default().contains("closed"),
        "detalhe precisa citar o estado: {body}"
    );
    assert_eq!(ack_rows(&pool, &id).await, 4, "409 não pode gravar ACK");
    assert_eq!(
        incident_state(&pool, &id).await.as_deref(),
        Some("closed"),
        "409 não pode mover o estado"
    );

    // A mesma prova por outra aresta: `replicate` a partir de `closed`.
    let res = post(addr, &format!("/v1/incidents/{id}/replicate"), Some(TOKEN_A), "");
    assert_eq!(res.status, 409, "replicate após close: {}", res.body);
    let body = res.json();
    assert_eq!(body["error"], "illegal_transition");
    assert_eq!(body["state"], "closed");

    // 19. incidente inexistente na leitura.
    let res = get(addr, &format!("/v1/incidents/{inexistente}"), Some(TOKEN_A));
    assert_eq!(res.status, 404, "inexistente precisa ser 404: {}", res.body);
    assert_eq!(res.json()["error"], "incident_not_found");

    // 19 (continuação). Incidente inexistente na transição: mesmo 404, nunca 409.
    let res = post(
        addr,
        &format!("/v1/incidents/{inexistente}/close"),
        Some(TOKEN_A),
        "",
    );
    assert_eq!(res.status, 404, "transição em inexistente: {}", res.body);
    assert_eq!(res.json()["error"], "incident_not_found");
    assert_eq!(
        incident_rows(&pool, &inexistente).await,
        0,
        "404 não pode criar incidente"
    );
    pool.close().await;
}

// --- verificações 9 a 13 e 16 -------------------------------------------------

/// Verificações 9–13 e 16: evento 201, replay 200, lacuna 409 nomeando a posição
/// esperada, classe inexistente 409, leitura paginada com `next_cursor` e cursor
/// ilegível 400. Cada recusa confere por SQL que nenhuma posição nova apareceu.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn evento_persistido_replay_lacuna_classe_e_cursor() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("stream");

    // 9. posição nova
    let body = event_body(&stream, 1, CLASS);
    let res = post(addr, "/v1/events", Some(TOKEN_A), &body);
    assert_eq!(res.status, 201, "posição nova precisa ser 201: {}", res.body);
    let ack = res.json();
    assert_eq!(ack["ack_class"], "persisted_local", "teto do ACK local");
    assert_eq!(ack["seq"], 1);
    assert_eq!(ack["retention_class"], CLASS);

    // 10. repetição da mesma posição
    let res = post(addr, "/v1/events", Some(TOKEN_A), &body);
    assert_eq!(res.status, 200, "replay precisa ser 200: {}", res.body);
    assert_eq!(res.json()["ack_class"], "persisted_local");
    assert_eq!(event_rows(&pool, TENANT_A, &stream).await, 1, "replay não duplica");

    // 11. salto de sequência: a resposta nomeia a posição esperada.
    let res = post(addr, "/v1/events", Some(TOKEN_A), &event_body(&stream, 9, CLASS));
    assert_eq!(res.status, 409, "lacuna precisa ser 409: {}", res.body);
    let gap = res.json();
    assert_eq!(gap["error"], "sequence_gap");
    assert!(
        gap["detail"].as_str().unwrap_or_default().contains("espera 2"),
        "a lacuna precisa nomear a posição esperada: {gap}"
    );
    assert_eq!(
        event_rows(&pool, TENANT_A, &stream).await,
        1,
        "lacuna não é saltada nem preenchida"
    );

    // 12. classe de retenção sem política na jurisdição.
    let res = post(
        addr,
        "/v1/events",
        Some(TOKEN_A),
        &event_body(&stream, 2, "classe-que-nao-existe"),
    );
    assert_eq!(res.status, 409, "classe inexistente precisa ser 409: {}", res.body);
    assert_eq!(res.json()["error"], "retention_policy_missing");
    assert_eq!(
        event_rows(&pool, TENANT_A, &stream).await,
        1,
        "classe inexistente não pode gravar posição"
    );

    // Posição 2 válida: dá duas linhas para a paginação abaixo.
    let res = post(addr, "/v1/events", Some(TOKEN_A), &event_body(&stream, 2, CLASS));
    assert_eq!(res.status, 201, "posição 2 precisa ser 201: {}", res.body);
    assert_eq!(event_rows(&pool, TENANT_A, &stream).await, 2);

    // 13. leitura paginada: `events[]` + `next_cursor` na primeira página.
    let path = format!(
        "/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=1"
    );
    let res = get(addr, &path, Some(TOKEN_A));
    assert_eq!(res.status, 200, "leitura autorizada: {}", res.body);
    let page = res.json();
    let events = page["events"].as_array().expect("events[] na página");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["seq"], 1);
    assert_eq!(events[0]["jurisdiction"], "BR");
    let cursor = page["next_cursor"]
        .as_str()
        .expect("há segunda página: next_cursor presente")
        .to_owned();

    // Retomada pelo cursor próprio: segunda página sem cursor novo.
    let path = format!(
        "/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=1&cursor={cursor}"
    );
    let res = get(addr, &path, Some(TOKEN_A));
    assert_eq!(res.status, 200, "cursor próprio é aceito: {}", res.body);
    let page = res.json();
    assert_eq!(page["events"].as_array().map(Vec::len), Some(1));
    assert_eq!(page["events"][0]["seq"], 2);
    assert!(page["next_cursor"].is_null(), "fim do intervalo: {page}");

    // 16. cursor ilegível é recusado com código próprio, não 500 nem 200.
    let path = format!(
        "/v1/events?stream_id={stream}&partition=1&authority_epoch=7&limit=1&cursor=nao-e-um-cursor"
    );
    let res = get(addr, &path, Some(TOKEN_A));
    assert_eq!(res.status, 400, "cursor ilegível precisa ser 400: {}", res.body);
    assert_eq!(res.json()["error"], "cursor_invalid");
    assert_eq!(
        event_rows(&pool, TENANT_A, &stream).await,
        2,
        "leitura recusada não pode gravar"
    );
    pool.close().await;
}

// --- verificações 14 e 15 (+ lacuna de Inbox com `expected`) ------------------

/// Verificações 14 e 15: lote de Inbox com 2 posições → `accepted=2`,
/// `contiguous_prefix=2`, `ack_class=persisted_local`; leitura do estado com
/// `gaps=[]`. Acrescenta a lacuna de Inbox, cujo corpo traz o campo `expected`
/// (a variante de `sequence_gap` que nomeia a posição esperada em JSON).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn inbox_aceita_duas_posicoes_e_le_o_prefixo() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(pool.clone()).await;
    let stream = unique("inbox");

    let posicao = |seq: u64| {
        json!({
            "origin_node": "node-e2e",
            "stream_id": stream,
            "partition": 1,
            "authority_epoch": 7,
            "seq": seq,
        })
    };
    let lote = json!({
        "retention_class": CLASS,
        "positions": [posicao(1), posicao(2)],
    })
    .to_string();

    // 14. lote contíguo de 2 posições.
    let res = post(addr, "/v1/sync/inbox", Some(TOKEN_A), &lote);
    assert_eq!(res.status, 200, "lote contíguo precisa ser 200: {}", res.body);
    let body = res.json();
    assert_eq!(body["accepted"], 2, "as duas posições foram aceitas: {body}");
    assert_eq!(body["duplicates"], 0);
    assert_eq!(body["contiguous_prefix"], 2);
    assert_eq!(body["ack_class"], "persisted_local", "teto do ACK local");
    assert_eq!(event_rows(&pool, TENANT_A, &stream).await, 2);

    // 15. estado do Inbox: prefixo 2, sem lacunas.
    let path =
        format!("/v1/sync/state?stream_id={stream}&partition=1&authority_epoch=7&origin_node=node-e2e");
    let res = get(addr, &path, Some(TOKEN_A));
    assert_eq!(res.status, 200, "estado do Inbox: {}", res.body);
    let state = res.json();
    assert_eq!(state["contiguous_prefix"], 2);
    assert_eq!(
        state["gaps"].as_array().map(Vec::len),
        Some(0),
        "inbox contíguo não tem lacuna: {state}"
    );
    assert!(state["last_recorded_at"].is_string(), "há instante durável: {state}");

    // Lacuna de Inbox: 409 com `expected` explícito e nada gravado além do
    // prefixo já durável.
    let salto = json!({
        "retention_class": CLASS,
        "positions": [posicao(5)],
    })
    .to_string();
    let res = post(addr, "/v1/sync/inbox", Some(TOKEN_A), &salto);
    assert_eq!(res.status, 409, "lacuna de Inbox precisa ser 409: {}", res.body);
    let gap = res.json();
    assert_eq!(gap["error"], "sequence_gap");
    assert_eq!(gap["expected"], 3, "o corpo nomeia a posição esperada: {gap}");
    assert_eq!(gap["received"], 5);
    assert_eq!(gap["accepted"], 0);
    assert_eq!(gap["contiguous_prefix"], 2);
    assert_eq!(
        event_rows(&pool, TENANT_A, &stream).await,
        2,
        "lacuna não pode ser saltada"
    );
    pool.close().await;
}

// --- verificações 17 e 18 -----------------------------------------------------

/// Verificações 17 e 18: token desconhecido → 401; credencial de outro tenant
/// **não registrada** → 401 (a barreira de credencial vem antes da tenancy); e
/// credencial de outro tenant **registrada** → 404 `incident_not_found` no dado
/// alheio, o mesmo 404 de incidente inexistente (existência não vaza).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn credencial_de_outro_tenant_nao_le_incidente_alheio() {
    let Some(env) = env().await else { return };
    let pool = pool(env).await;
    let addr = serve(pool.clone()).await;
    let id = unique("tenancy");

    let res = post(addr, "/v1/incidents", Some(TOKEN_A), &incident_body(&id));
    assert_eq!(res.status, 201, "preparação: {}", res.body);

    // 17. token que não pertence a nenhum grant.
    let res = post(addr, "/v1/incidents", Some(TOKEN_DESCONHECIDO), &incident_body(&unique("x")));
    assert_eq!(res.status, 401, "token desconhecido precisa ser 401: {}", res.body);
    assert_eq!(res.json()["error"], "unauthenticated");

    // 18a. credencial de outro tenant fora do registro: 401, como no roteiro
    // manual — a credencial inválida barra antes de qualquer avaliação de
    // tenancy, e nada é lido.
    let res = get(addr, &format!("/v1/incidents/{id}"), Some(TOKEN_B_NAO_REGISTRADO));
    assert_eq!(res.status, 401, "credencial fora do registro: {}", res.body);
    assert_eq!(res.json()["error"], "unauthenticated");

    // 18b. credencial **válida** do tenant B: o incidente do tenant A responde
    // 404, não 403 — a existência do identificador alheio não é confirmada.
    let res = get(addr, &format!("/v1/incidents/{id}"), Some(TOKEN_B));
    assert_eq!(res.status, 404, "dado de outro tenant é 404: {}", res.body);
    assert_eq!(res.json()["error"], "incident_not_found");
    assert!(
        !res.body.contains(TENANT_A),
        "a resposta não pode nomear o tenant dono: {}",
        res.body
    );

    // A mesma tenancy nas transições: o tenant B não move o incidente do A.
    let res = post(addr, &format!("/v1/incidents/{id}/close"), Some(TOKEN_B), "");
    assert_eq!(res.status, 404, "transição cross-tenant é 404: {}", res.body);
    assert_eq!(
        incident_state(&pool, &id).await.as_deref(),
        Some("persisted_local"),
        "recusa cross-tenant não pode mover o estado"
    );
    assert_eq!(ack_rows(&pool, &id).await, 1, "recusa cross-tenant não grava ACK");

    // Controle positivo: o dono lê o próprio incidente.
    let res = get(addr, &format!("/v1/incidents/{id}"), Some(TOKEN_A));
    assert_eq!(res.status, 200, "o dono precisa ler o próprio dado: {}", res.body);
    let body = res.json();
    assert_eq!(body["incident_id"], id.as_str());
    assert_eq!(body["state"], "persisted_local");
    pool.close().await;
}

// --- verificação 21 -----------------------------------------------------------

/// Caminho de identidade de nó para o binário (`GODMESH_NODE_KEY`).
///
/// Desde a fatia de federação o binário exige identidade persistida para servir.
/// Deixamos o caminho **inexistente** de propósito: o binário gera a chave com
/// `O_EXCL` e modo 0600, que é o caminho de produção — criar o arquivo vazio aqui
/// faria o carregador recusar ("esperados 32 bytes, obtidos 0").
fn node_key_file() -> PathBuf {
    let caminho = std::env::temp_dir().join(format!("godmesh-e2e-node-{}.key", std::process::id()));
    let _ = std::fs::remove_file(&caminho);
    caminho
}

/// Diretório de migrações do repositório, independente do `cwd` do `cargo test`.
fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("migrations")
        .canonicalize()
        .expect("diretório migrations do repositório")
}

/// Registro de grants no formato de produção (`GODMESH_GRANTS`), para o binário.
fn grants_json() -> String {
    json!({
        "grants": [
            {
                "token": TOKEN_A,
                "token_id": "g-e2e-a",
                "tenant_id": TENANT_A,
                "principal": "principal-tenant-e2e-br-a",
                "scopes": ["incident:write", "incident:read", "event:write", "event:read", "sync:write", "sync:read"],
                "not_before": 0,
                "expires_at": 4102444800_i64,
                "authority_epoch": 7,
                "operation": "open_sos_incident",
                "freshness": "f3_verified_online",
                "revocation": "absent",
                "epoch_comparable": true,
                "action_enumerated": true,
                "retention_class_known": true
            }
        ]
    })
    .to_string()
}

/// Processo filho do teste 21. Se o teste falhar antes do `SIGTERM`, o filho é
/// morto no `Drop`: um teste vermelho não pode deixar servidor escutando (nem
/// segurando conexões no banco descartável) depois de terminar.
struct Filho {
    processo: Option<std::process::Child>,
}

impl Filho {
    fn novo(processo: std::process::Child) -> Self {
        Self {
            processo: Some(processo),
        }
    }

    fn processo(&mut self) -> &mut std::process::Child {
        self.processo.as_mut().expect("processo filho ativo")
    }

    /// Chamado depois do encerramento confirmado: não há o que matar.
    fn desarmar(&mut self) {
        self.processo = None;
    }
}

impl Drop for Filho {
    fn drop(&mut self) {
        if let Some(mut processo) = self.processo.take() {
            let _ = processo.kill();
            let _ = processo.wait();
        }
    }
}

/// Verificação 21: o binário de verdade encerra por `SIGTERM` com código 0,
/// drenando o que estava em voo e registrando o encerramento.
///
/// Roda contra o mesmo banco descartável da suíte (o binário reaplica as
/// migrações, o que também prova a idempotência do runner sobre o banco do E2E).
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn binario_encerra_por_sigterm_com_codigo_zero() {
    let Some(env) = env().await else { return };

    let mut filho = Filho::novo(
        Command::new(env!("CARGO_BIN_EXE_godmesh-server"))
            .env("DATABASE_URL", &env.url)
            .env("GODMESH_MIGRATIONS_DIR", migrations_dir())
            .env("GODMESH_GRANTS", grants_json())
            .env("GODMESH_ENV", "dev")
            .env("BIND_ADDR", "127.0.0.1:0")
            .env("GODMESH_NODE_KEY", node_key_file())
            .env("GODMESH_OUTBOX", "off")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sobe o binário do servidor"),
    );

    let mut log = String::new();
    let mut linhas = BufReader::new(filho.processo().stdout.take().expect("stdout")).lines();
    let mut endereco = None;
    for _ in 0..100 {
        match linhas.next() {
            Some(Ok(linha)) => {
                log.push_str(&linha);
                log.push('\n');
                if let Some(resto) = linha.split("escutando em http://").nth(1) {
                    // A linha continua com " (N grant(s) configurado(s))".
                    endereco = resto
                        .split_whitespace()
                        .next()
                        .and_then(|endereco| endereco.parse::<SocketAddr>().ok());
                    break;
                }
            }
            Some(Err(e)) => panic!("stdout ilegível: {e}"),
            None => break,
        }
    }
    let addr = endereco.unwrap_or_else(|| panic!("o binário não chegou a escutar:\n{log}"));

    // O processo está de fato servindo antes do sinal.
    let res = get(addr, "/healthz", None);
    assert_eq!(res.status, 200, "healthz do binário: {}", res.body);
    assert!(res.body.contains("\"status\":\"ok\""), "corpo: {}", res.body);

    let pid = filho.processo().id();
    let sinal = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("envia SIGTERM");
    assert!(sinal.success(), "kill -TERM falhou: {sinal}");

    let mut codigo = None;
    for _ in 0..100 {
        if let Some(status) = filho.processo().try_wait().expect("try_wait") {
            codigo = Some(status.code());
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let codigo = codigo.expect("processo não encerrou em 10s após SIGTERM");
    for linha in linhas.map_while(Result::ok) {
        log.push_str(&linha);
        log.push('\n');
    }
    let mut erros = String::new();
    if let Some(mut stderr) = filho.processo().stderr.take() {
        let _ = stderr.read_to_string(&mut erros);
    }
    // O processo já terminou; o guard não precisa mais matar nada.
    filho.desarmar();

    assert_eq!(codigo, Some(0), "SIGTERM precisa sair com 0; log:\n{log}\n{erros}");
    assert!(
        log.contains("em voo drenada"),
        "o log precisa trazer a contagem drenada:\n{log}"
    );
    assert!(
        log.contains("encerrado"),
        "o log precisa registrar o encerramento:\n{log}"
    );
}