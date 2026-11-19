//! Teste de contrato (ADR-074): o documento **não pode mentir**.
//!
//! Três autoridades comparadas, todas lidas do servidor real:
//!  1. o router montado pelo `Registry` (o que o axum atende);
//!  2. `GET /v1/meta` → `routes[]`/`errors[]` (o que um agente descobre sem
//!     credencial);
//!  3. `GET /openapi.json` (o documento gerado do mesmo registro).
//!
//! A regra que o CTO pediu — "teste deve falhar se alguém adicionar rota sem
//! documentar" — vale por dois caminhos independentes:
//!  * **estrutural**: router, meta e openapi nascem do mesmo `Registry`; não
//!    existe segunda lista para divergir, e `rota_fora_do_registro_nao_compila`
//!    varre o código-fonte para garantir que ninguém chamou `.route()` à mão;
//!  * **comportamental**: cada operação do documento é sondada contra o
//!    router real (401 no portão protegido, resposta própria no público); um
//!    documento que anunciar rota que o router não monta, ou omitir rota que
//!    ele monta, falha aqui.
//!
//! Além disso: todo código de erro documentado existe no catálogo com o mesmo
//! status HTTP; o corpo de erro real carrega `hint`/`docs_url`/`retryable`;
//! `dry_run` em rota destrutiva **não escreve nada** (prova por contagem SQL
//! antes/depois, com banco descartável); `Idempotency-Key` ausente continua
//! aceito e cabeçalho inválido é 422 antes de qualquer efeito.
//!
//! Execução (local-first, sem contêiner):
//!
//! ```text
//! DATABASE_URL=postgres://godmesh@localhost:55499/godmesh_gate \
//!   cargo test -p godmesh-server --test api_contract
//! ```
//!
//! Sem `DATABASE_URL` os testes que exigem banco são pulados com aviso (não
//! fingem sucesso); os testes de documento correm sem banco.

mod common;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_identity::grant::{
    FreshnessSource, GrantPredicate, OfflineOp, RevocationState,
};
use godmesh_server::auth::{
    Authorization, GrantRecord, GrantRegistry, Scope, Subject,
};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// Total de rotas publicadas no contrato. Mudar o número **exige** mudar o
/// contrato com ele: é o grito pedido pelo CTO quando alguém registra uma
/// rota e "esquece" de documentá-la.
const EXPECTED_ROUTE_COUNT: usize = 33;

const TENANT: &str = "tenant-contrato-br";
const TOKEN_ALL: &str = "token-contrato-todos-escopos";

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(label: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{label}-{}-{n}", std::process::id())
}

// --- ambiente ----------------------------------------------------------------

/// Servidor com pool **preguiçoso** (banco pode estar morto): os testes de
/// documento não precisam de Postgres — o portão responde antes do handler.
async fn serve_sem_banco() -> SocketAddr {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_millis(300))
        .connect_lazy("postgres://godmesh@127.0.0.1:59999/inalcancavel")
        .expect("pool preguiçoso não conecta até a primeira query");
    let registry = GrantRegistry::from_entries([]);
    let app = godmesh_server::http::router(pool, registry);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

struct Env {
    url: String,
}

static ENV: tokio::sync::OnceCell<Env> = tokio::sync::OnceCell::const_new();

fn db_name() -> String {
    format!("godmesh_contract_{}", std::process::id())
}

fn with_database(url: &str, db: &str) -> String {
    let corte = url.rfind('/').expect("DATABASE_URL termina em /banco");
    format!("{}{db}", &url[..corte + 1])
}

async fn setup() -> Env {
    let base = common::database_url().expect("URL presente (verificada antes)");
    let db = db_name();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_database(&base, "postgres"))
        .await
        .unwrap_or_else(|e| panic!("Postgres inacessível para administração: {e}"));
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {db} WITH (FORCE)"
    )))
    .execute(&admin)
    .await
    .expect("drop do banco descartável");
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {db}")))
        .execute(&admin)
    .await
    .expect("create do banco descartável");
    admin.close().await;
    let url = with_database(&base, &db);
    let pool = common::pool_para(&url, 4).await;
    // Jurisdição + política para o ciclo de incidente usado nos dry_run.
    sqlx::query(
        "INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, 'BR') \
         ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction",
    )
    .bind(TENANT)
    .execute(&pool)
    .await
    .expect("jurisdição BR");
    sqlx::query(
        "INSERT INTO retention_class (class, jurisdiction, finality, months, discard_trigger) \
         SELECT 'incidente_sos', 'BR', 'dever_de_cuidado', 12, 'incident_closed + 12m' \
          WHERE NOT EXISTS (SELECT 1 FROM retention_class \
                             WHERE class = 'incidente_sos' AND jurisdiction = 'BR')",
    )
    .execute(&pool)
    .await
    .expect("classe incidente_sos");
    sqlx::query(
        "INSERT INTO retention_copy (class, jurisdiction, copy_scope) \
         SELECT 'incidente_sos', 'BR', 'primary' \
          WHERE NOT EXISTS (SELECT 1 FROM retention_copy \
                             WHERE class = 'incidente_sos' AND jurisdiction = 'BR' \
                               AND copy_scope = 'primary')",
    )
    .execute(&pool)
    .await
    .expect("cópia primária");
    drop(pool);
    Env { url }
}

async fn env() -> Option<&'static Env> {
    if common::database_url().is_none() {
        eprintln!(
            "SKIP: sem GODMESH_TEST_DATABASE_URL/DATABASE_URL; os testes de efeito (dry_run, \
             idempotência com escrita) dependem de Postgres"
        );
        return None;
    }
    Some(ENV.get_or_init(setup).await)
}

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

fn record(token_id: &str, scopes: Vec<Scope>) -> GrantRecord {
    GrantRecord {
        token_id: token_id.to_owned(),
        subject: Subject {
            tenant_id: TENANT.to_owned(),
            principal: "operador-contrato".to_owned(),
        },
        scopes,
        not_before: 0,
        expires_at: i64::MAX,
        revoked_at: None,
        authority_epoch: 7,
        predicate: predicate(),
    }
}

async fn serve_com_banco() -> SocketAddr {
    let env = env().await.expect("banco presente");
    let pool = common::pool_para(&env.url, 4).await;
    let registry = GrantRegistry::from_records([(
        TOKEN_ALL.to_owned(),
        record("g-contrato", Scope::ALL.to_vec()),
    )]);
    let app = godmesh_server::http::router_with(pool.clone(), Authorization::new(registry, pool));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

async fn pool() -> PgPool {
    let env = env().await.expect("banco presente");
    common::pool_para(&env.url, 4).await
}

// --- cliente HTTP/1.1 sem dependência nova (padrão dos testes irmãos) --------

struct Response {
    status: u16,
    body: String,
}

impl Response {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| {
            panic!(
                "corpo não é JSON ({e}): {:?} (status {})",
                self.body, self.status
            )
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn http_call(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
    extra_headers: &[(&str, &str)],
) -> Response {
    let mut stream = TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {addr}\r\n");
    if let Some(value) = token {
        request.push_str(&format!("Authorization: Bearer {value}\r\n"));
    }
    if body.is_some() {
        request.push_str("Content-Type: application/json\r\n");
    }
    for (name, value) in extra_headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(body) = body {
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
    let status: u16 = head
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {raw:?}"));
    Response {
        status,
        body: body.to_owned(),
    }
}

fn get(addr: SocketAddr, path: &str, token: Option<&str>) -> Response {
    http_call(addr, "GET", path, token, None, &[])
}

fn post(addr: SocketAddr, path: &str, token: Option<&str>, body: &str) -> Response {
    http_call(addr, "POST", path, token, Some(body), &[])
}

fn meta(addr: SocketAddr) -> Value {
    let res = get(addr, "/v1/meta", None);
    assert_eq!(res.status, 200, "/v1/meta é público: {}", res.body);
    res.json()
}

fn openapi(addr: SocketAddr) -> Value {
    let res = get(addr, "/openapi.json", None);
    assert_eq!(res.status, 200, "/openapi.json é público: {}", res.body);
    res.json()
}

/// (método, caminho, escopo) a partir do `routes[]` do meta.
fn meta_routes(meta: &Value) -> Vec<(String, String, Option<String>)> {
    meta["routes"]
        .as_array()
        .expect("routes é array")
        .iter()
        .map(|r| {
            (
                r["method"].as_str().unwrap().to_owned(),
                r["path"].as_str().unwrap().to_owned(),
                r["scope"].as_str().map(str::to_owned),
            )
        })
        .collect()
}

/// (método, caminho, escopo citado na descrição) a partir do documento.
fn doc_routes(doc: &Value) -> Vec<(String, String, Option<String>)> {
    let mut out = Vec::new();
    let paths = doc["paths"].as_object().expect("paths é objeto");
    for (path, item) in paths {
        for (method, op) in item.as_object().expect("path item é objeto") {
            if !matches!(method.as_str(), "get" | "post") {
                continue;
            }
            let desc = op["description"].as_str().unwrap_or("");
            let scope = desc
                .split("Escopo exigido: `")
                .nth(1)
                .and_then(|rest| rest.split('`').next())
                .map(str::to_owned);
            out.push((method.to_uppercase(), path.clone(), scope));
        }
    }
    out
}

fn key(triples: &[(String, String, Option<String>)]) -> std::collections::BTreeSet<String> {
    // Normaliza por "MÉTODO caminho" (escopo conferido à parte).
    triples
        .iter()
        .map(|(m, p, _)| format!("{m} {p}"))
        .collect()
}

// ---------------------------------------------------------------------------
// 1) Documento × router × meta: a mesma tabela, três leituras.
// ---------------------------------------------------------------------------

# [tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn contrato_publicado_e_o_mesmo_registro_do_router() {
    let addr = serve_sem_banco().await;
    let meta = meta(addr);
    let doc = openapi(addr);

    assert_eq!(meta["api_version"], "v1");
    assert!(
        meta["supported_versions"]
            .as_array()
            .is_some_and(|v| !v.is_empty()),
        "supported_versions vazio não informa N/N-1"
    );

    let routes = meta_routes(&meta);
    let docs = doc_routes(&doc);
    assert_eq!(
        routes.len(),
        EXPECTED_ROUTE_COUNT,
        "o contrato publishou {} rotas; o esperado é {EXPECTED_ROUTE_COUNT} — mudou a API, \
         atualize o contrato **e** a contagem deliberadamente",
        routes.len()
    );
    assert_eq!(
        docs.len(),
        EXPECTED_ROUTE_COUNT,
        "openapi.json divergiu do meta: documento está mentindo sobre o tamanho da API"
    );
    assert_eq!(
        key(&routes),
        key(&docs),
        "conjunto de (método, caminho) do meta ≠ do openapi"
    );

    for (method, suffix) in [
        ("GET", "/local-manifest"), ("POST", "/genesis-imports"),
        ("POST", "/join-signatures"), ("POST", "/join-requests/{join_request_id}/approve-v2"),
        ("GET", "/join-requests/{join_request_id}/proof"), ("POST", "/join-validity"),
        ("POST", "/join-imports"), ("POST", "/join-revocations"),
        ("POST", "/join-revocation-imports"),
    ] {
        let path = format!("/v1/federation{suffix}");
        assert!(routes.iter().any(|(verb, route, scope)| verb == method && route == &path
            && scope.as_deref() == Some("federation:admin")), "{method} {path}");
    }

    // Escopo idêntico nas duas leituras.
    for (method, path, scope) in &routes {
        let (_, _, doc_scope) = docs
            .iter()
            .find(|(m, p, _)| m == method && p == path)
            .unwrap_or_else(|| panic!("rota {method} {path} sumiu do documento"));
        assert_eq!(
            scope.as_deref(),
            doc_scope.as_deref(),
            "escopo declarado diverge entre meta e openapi em {method} {path}"
        );
    }

    // Sondagem contra o router REAL: documento que promete rota que o router
    // não atende falha aqui, rota por rota.
    for (method, path, scope) in &routes {
        let full_path = exemplo_de_path(path);
        let res = match method.as_str() {
            "GET" => get(addr, &full_path, None),
            _ => post(addr, &full_path, None, "{}"),
        };
        if scope.is_some() {
            assert_eq!(
                res.status, 401,
                "{method} {path} é protegida e precisa esbarrar no portão (não 404!): {}",
                res.body
            );
            assert_eq!(res.json()["error"], "unauthenticated");
        } else {
            assert!(
                !matches!(res.status, 404 | 405),
                "rota pública {method} {path} não existe no router: status {}",
                res.status
            );
        }
    }

    // O inverso estrutural: rota fora do registro não existe (o `Registry`
    // é a única fonte); e rota inventada pelo cliente morre em 404.
    let inexistente = get(addr, "/v1/rota-que-ninguem-registrou", None);
    assert_eq!(inexistente.status, 404);
    let metodo_errado = http_call(addr, "DELETE", "/v1/incidents/x", None, None, &[]);
    assert!(
        matches!(metodo_errado.status, 401 | 405),
        "DELETE não registrado não pode parecer rota viva: {}",
        metodo_errado.status
    );
}

/// Expande {param} para um valor de sondagem (a rota casa por prefixo no
/// matchit; o portão roda antes do extrator de path).
fn exemplo_de_path(path: &str) -> String {
    path.replace("{id}", "sonda")
        .replace("{token_id}", "sonda")
        .replace("{node_id}", "sonda")
        .replace("{join_request_id}", "sonda")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn catalogo_de_erros_cobre_todo_codigo_documentado() {
    let addr = serve_sem_banco().await;
    let meta = meta(addr);
    let doc = openapi(addr);

    let catalogo: Vec<&Value> = meta["errors"].as_array().expect("errors é array").iter().collect();
    assert!(
        catalogo.len() >= 25,
        "catálogo raso demais para a superfície de ApiError: {}",
        catalogo.len()
    );
    for entry in &catalogo {
        assert!(entry["error"].is_string(), "cada erro nomeia o código");
        assert!(entry["http_status"].is_number(), "e o status");
        assert!(entry["when"].is_string(), "e quando ocorre");
        assert!(entry["retryable"].is_boolean(), "e se retry ajuda");
        assert!(
            entry["hint"].as_str().is_some_and(|h| !h.is_empty()),
            "hint vazio não é acionável: {entry}"
        );
    }

    // Todo 4xx/5xx do documento aponta para um código do catálogo **com o
    // mesmo status**. Sem isso, o exemplo é ficção.
    for (path, item) in doc["paths"].as_object().unwrap() {
        for (_method, op) in item.as_object().unwrap() {
            let Some(responses) = op["responses"].as_object() else {
                continue;
            };
            for (status, response) in responses {
                let status: u16 = status.parse().expect("status numérico");
                if status < 400 || *path == "/healthz" {
                    // 503 do healthz não é um erro da API: é o corpo
                    // {status, dependency, ack_class} de liveness.
                    continue;
                }
                let desc = response["description"].as_str().unwrap_or("");
                let code = desc
                    .split('`')
                    .nth(1)
                    .unwrap_or_else(|| panic!("resposta {status} sem código em backticks: {desc}"));
                let entry = catalogo
                    .iter()
                    .find(|e| e["error"] == code)
                    .unwrap_or_else(|| panic!("código {code} documentado fora do catálogo"));
                assert_eq!(
                    entry["http_status"].as_u64().unwrap() as u16,
                    status,
                    "código {code} publicado com status ≠ do documento"
                );
                // exemplo realista no corpo da resposta documentada
                let example = &response["content"]["application/json"]["example"];
                assert_eq!(example["error"], code);
                assert!(example["hint"].is_string(), "exemplo precisa carregar hint");
                assert!(example["docs_url"].is_string());
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn corpo_de_erro_real_e_acionavel_e_estavel() {
    let addr = serve_sem_banco().await;
    let res = get(addr, "/v1/incidents", None);
    assert_eq!(res.status, 401);
    let body = res.json();
    // código histórico intacto (testado pelas suítes existentes):
    assert_eq!(body["error"], "unauthenticated");
    assert!(body["detail"].is_string());
    assert_eq!(body["ack_class"], "transport_accepted");
    // campos novos da fatia:
    assert!(
        body["hint"].as_str().is_some_and(|h| !h.is_empty()),
        "hint é o próximo passo concreto: {body}"
    );
    assert_eq!(body["docs_url"], "/v1/meta");
    assert!(body["retryable"].is_boolean());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn documento_declara_bearer_e_ref_resolvem() {
    let addr = serve_sem_banco().await;
    let doc = openapi(addr);
    assert_eq!(doc["openapi"], "3.1.0");
    let scheme = &doc["components"]["securitySchemes"]["bearer"];
    assert_eq!(scheme["type"], "http");
    assert_eq!(scheme["scheme"], "bearer");

    let schemas = doc["components"]["schemas"]
        .as_object()
        .expect("schemas")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut refs = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(map) => {
                for (k, child) in map {
                    if k == "$ref" {
                        if let Some(s) = child.as_str() {
                            out.push(s.to_owned());
                        }
                    } else {
                        walk(child, out);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
            _ => {}
        }
    }
    walk(&doc, &mut refs);
    for reference in refs {
        let name = reference
            .strip_prefix("#/components/schemas/")
            .unwrap_or_else(|| panic!("$ref fora de components/schemas: {reference}"));
        assert!(
            schemas.iter().any(|s| s == name),
            "documento referencia schema inexistente: {name}"
        );
    }

    // Toda rota protegida do documento exige o bearer; nenhuma pública exige.
    for (_path, item) in doc["paths"].as_object().unwrap() {
        for (method, op) in item.as_object().unwrap() {
            let _ = method;
            let protected = op["description"]
                .as_str()
                .is_some_and(|d| d.contains("Escopo exigido: `"));
            let security = op["security"].as_array();
            if protected {
                assert!(
                    security.is_some_and(|s| !s.is_empty()),
                    "operação protegida sem security: {op}"
                );
            } else {
                assert!(
                    security.map(|s| s.is_empty()).unwrap_or(true),
                    "operação pública com exigência de credencial: {op}"
                );
            }
        }
    }
}

/// Guarda do bloco `capacity` do `/healthz` (ADR-073 camada 1): o corpo real
/// e o schema do documento são lidos do **mesmo** servidor e comparados chave
/// por chave, em dois níveis. Se o handler publicar `capacity` sem o campo
/// estar no `HealthResponse` do contrato (ou o inverso), este teste cai — é a
/// regra da ADR-074: documento que mente derruba o teste.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn corpo_de_healthz_bate_com_o_schema_do_documento() {
    let addr = serve_sem_banco().await;
    let doc = openapi(addr);
    let schema = &doc["components"]["schemas"]["HealthResponse"];
    let props = schema["properties"]
        .as_object()
        .expect("HealthResponse.properties");
    assert!(
        props.contains_key("capacity"),
        "o contrato não declara o bloco capacity do /healthz"
    );
    let capacity_props = props["capacity"]["properties"]
        .as_object()
        .expect("capacity é objeto inline com propriedades declaradas");
    for campo in [
        "cpu_pressure",
        "memory_pressure",
        "disk_free_ratio",
        "source",
        "container",
        "snapshot_age_seconds",
        "pressure_is_not_a_health_signal",
    ] {
        assert!(
            capacity_props.contains_key(campo),
            "capacity.{campo} publicado pelo handler e ausente do schema"
        );
    }

    // E o corpo real (aqui o 503 honesto, com banco preguiçoso morto) só tem
    // chaves declaradas, com o bloco de capacidade presente nos dois casos.
    let res = get(addr, "/healthz", None);
    assert!(
        res.status == 200 || res.status == 503,
        "healthz fora do contrato: {}",
        res.status
    );
    let body = res.json();
    for chave in body.as_object().expect("corpo é objeto").keys() {
        assert!(
            props.contains_key(chave),
            "/healthz publica {chave} que o documento não declara"
        );
    }
    let capacity = body["capacity"]
        .as_object()
        .unwrap_or_else(|| panic!("/healthz sem bloco capacity: {body}"));
    for chave in capacity.keys() {
        assert!(
            capacity_props.contains_key(chave),
            "capacity.{chave} não declarada no schema do documento"
        );
    }
    for campo in [
        "source",
        "container",
        "pressure_is_not_a_health_signal",
    ] {
        assert!(
            capacity.contains_key(campo),
            "capacity.{campo} obrigatória por contrato faltando no corpo"
        );
    }
    assert_eq!(
        capacity["pressure_is_not_a_health_signal"], true,
        "pressão não julga saúde nesta fatia e o corpo tem que dizer isso"
    );
}

/// Guarda estrutural do registro: nenhuma rota pode nascer de `.route()` à
/// mão fora do `Registry`. Se alguém voltar a compor router diretamente,
/// este teste falha — é a garantia de que o contrato não pode ficar para
/// trás por esquecimento.
#[test]
fn rota_fora_do_registro_nao_compila() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/http");
    // Só `contract.rs` chama `.route()`: é onde o registro vira router. Os
    // demais módulos (inclusive a composição em `mod.rs`) não têm esse poder.
    // `mod.rs` pode aplicar `route_layer` (o portão de autenticação do grupo
    // protegido), mas jamais registrar uma rota.
    let permitidos = ["contract.rs"];
    for entry in std::fs::read_dir(dir).expect("diretório http existe") {
        let path = entry.expect("entrada").path();
        let nome = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !nome.ends_with(".rs") || permitidos.contains(&nome) {
            continue;
        }
        let fonte = std::fs::read_to_string(&path).expect("fonte legível");
        for (i, linha) in fonte.lines().enumerate() {
            // comentário não registra rota; só código conta.
            if linha.trim_start().starts_with("//") {
                continue;
            }
            let rota_manual = linha.contains(".route(");
            let camada_fora_de_mod =
                linha.contains(".route_layer(") && nome != "mod.rs";
            assert!(
                !rota_manual && !camada_fora_de_mod,
                "{}:{}: rota montada fora do Registry — registre em \
                 `contract::Registry` para que /v1/meta e openapi.json nasçam \
                 com ela",
                nome,
                i + 1
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 2) Semântica segura para agente, provada com banco real.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dry_run_em_rota_destrutiva_nao_escreve_nada() {
    let Some(_env) = env().await else { return };
    let addr = serve_com_banco().await;
    let pool = pool().await;

    // Incidente vivo até `acknowledged_human`: o `close` seria legal.
    let id = unique("dry-close");
    let criado = post(
        addr,
        "/v1/incidents",
        Some(TOKEN_ALL),
        &json!({ "incident_id": id }).to_string(),
    );
    assert_eq!(criado.status, 201, "{}", criado.body);
    for step in ["replicate", "ack-technical"] {
        let r = post(addr, &format!("/v1/incidents/{id}/{step}"), Some(TOKEN_ALL), "");
        assert_eq!(r.status, 200, "{step}: {}", r.body);
    }
    let r = post(
        addr,
        &format!("/v1/incidents/{id}/acknowledge"),
        Some(TOKEN_ALL),
        &json!({ "responder": "pessoa-de-teste" }).to_string(),
    );
    assert_eq!(r.status, 200, "{}", r.body);

    let antes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sos_ack WHERE incident_id = $1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .expect("contagem da trilha");
    assert!(antes > 0, "trilha de ACK existe antes do dry_run");

    // dry_run: 200, `applied: false`, nada escrito.
    let dry = post(
        addr,
        &format!("/v1/incidents/{id}/close?dry_run=true"),
        Some(TOKEN_ALL),
        "",
    );
    assert_eq!(dry.status, 200, "dry_run precisa ser 200: {}", dry.body);
    let corpo = dry.json();
    assert_eq!(corpo["applied"], false);
    assert!(corpo["would_do"].is_array());
    let depois: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sos_ack WHERE incident_id = $1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .expect("contagem da trilha depois");
    assert_eq!(antes, depois, "dry_run escreveu na trilha de ACK");
    let estado: String = sqlx::query_scalar(
        "SELECT state FROM sos_incident WHERE id = $1 AND tenant_id = $2",
    )
    .bind(&id)
    .bind(TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(estado, "acknowledged_human", "dry_run fechou o incidente");

    // E o close real fecha: o caminho não virou no-op permanente.
    let fecha = post(
        addr,
        &format!("/v1/incidents/{id}/close"),
        Some(TOKEN_ALL),
        "",
    );
    assert_eq!(fecha.status, 200, "{}", fecha.body);
    let depois_close: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sos_ack WHERE incident_id = $1",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let estado_fim: String = sqlx::query_scalar(
        "SELECT state FROM sos_incident WHERE id = $1 AND tenant_id = $2",
    )
    .bind(&id)
    .bind(TENANT)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        estado_fim, "closed",
        "o caminho real continua vivo: close fecha (o dry_run não fechava)"
    );
    let _ = depois_close;

    // Revogação: mesma prova, em outra tabela.
    let emitido = post(
        addr,
        "/v1/admin/grants",
        Some(TOKEN_ALL),
        &json!({
            "tenant_id": TENANT,
            "principal": "vitalicio",
            "scopes": ["incident:read"],
            "ttl_seconds": 3600,
        })
        .to_string(),
    );
    assert_eq!(emitido.status, 201, "{}", emitido.body);
    let token_id = emitido.json()["token_id"].as_str().unwrap().to_owned();
    let revogadas_antes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM grant_revocation")
            .fetch_one(&pool)
            .await
            .unwrap();
    let dry = post(
        addr,
        &format!("/v1/admin/grants/{token_id}/revoke?dry_run=true"),
        Some(TOKEN_ALL),
        &json!({ "reason": "sonda de contrato" }).to_string(),
    );
    assert_eq!(dry.status, 200, "{}", dry.body);
    assert_eq!(dry.json()["applied"], false);
    let revogadas_depois: i64 =
        sqlx::query_scalar("SELECT count(*) FROM grant_revocation")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        revogadas_antes, revogadas_depois,
        "dry_run de revogação escreveu em grant_revocation"
    );
    let real = post(
        addr,
        &format!("/v1/admin/grants/{token_id}/revoke"),
        Some(TOKEN_ALL),
        &json!({ "reason": "close do contrato" }).to_string(),
    );
    assert_eq!(real.status, 200, "{}", real.body);
    let revogadas_fim: i64 =
        sqlx::query_scalar("SELECT count(*) FROM grant_revocation")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revogadas_fim, revogadas_antes + 1, "revogação real não registrou");

    pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn idempotency_key_fronteira_e_dry_run_em_mutante() {
    let Some(_env) = env().await else { return };
    let addr = serve_com_banco().await;
    let pool = pool().await;

    // Ausente: compatibilidade — continua aceito (nada quebra hoje).
    let sem_chave = post(
        addr,
        "/v1/incidents",
        Some(TOKEN_ALL),
        &json!({ "incident_id": unique("idem-ok") }).to_string(),
    );
    assert_eq!(sem_chave.status, 201, "{}", sem_chave.body);

    // Válido: aceito (a deduplicação do efeito é a próxima fatia; a
    // fronteira já vale).
    let com_chave = http_call(
        addr,
        "POST",
        "/v1/incidents",
        Some(TOKEN_ALL),
        Some(&json!({ "incident_id": unique("idem-key") }).to_string()),
        &[("Idempotency-Key", "req-7f3a-0001")],
    );
    assert_eq!(com_chave.status, 201, "{}", com_chave.body);

    // 129 bytes: recusado na fronteira.
    let longa = "a".repeat(129);
    let res = http_call(
        addr,
        "POST",
        "/v1/incidents",
        Some(TOKEN_ALL),
        Some(&json!({ "incident_id": unique("idem-longa") }).to_string()),
        &[("Idempotency-Key", longa.as_str())],
    );
    assert_eq!(res.status, 422, "{}", res.body);
    assert_eq!(res.json()["error"], "invalid_request");
    assert!(
        res.json()["detail"]
            .as_str()
            .is_some_and(|d| d.contains("Idempotency-Key")),
        "detail precisa nomear o cabeçalho recusado: {}",
        res.body
    );

    // Charset errado (espaço/UTF-8) também.
    let res = http_call(
        addr,
        "POST",
        "/v1/incidents",
        Some(TOKEN_ALL),
        Some(&json!({ "incident_id": unique("idem-charset") }).to_string()),
        &[("Idempotency-Key", "chave com espaco")],
    );
    assert_eq!(res.status, 422, "{}", res.body);

    // Nada dos pedidos recusados virou linha no banco. Contagem por prefixo
    // próprio: testes do mesmo binário dividem o banco descartável.
    async fn contagem(pool: &PgPool) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM sos_incident WHERE tenant_id = $1 AND id LIKE 'idem-%'",
        )
        .bind(TENANT)
        .fetch_one(pool)
        .await
        .unwrap()
    }
    assert_eq!(contagem(&pool).await, 2, "recusa de fronteira escreveu incidente");

    // dry_run=true em rota mutante **não destrutiva**: 422 declarado, não
    // ignorado em silêncio.
    let res = post(
        addr,
        "/v1/incidents?dry_run=true",
        Some(TOKEN_ALL),
        &json!({ "incident_id": unique("dry-mutante") }).to_string(),
    );
    assert_eq!(res.status, 422, "{}", res.body);
    assert_eq!(contagem(&pool).await, 2, "dry_run em não-destrutiva escreveu");

    // Valor malformado do parâmetro: recusado (sem sinonimia silenciosa).
    let res = post(
        addr,
        "/v1/incidents/x/close?dry_run=1",
        Some(TOKEN_ALL),
        "",
    );
    assert_eq!(res.status, 422, "{}", res.body);

    // Rota pública mutante: o portão da chave roda antes do manifesto.
    let res = http_call(
        addr,
        "POST",
        "/v1/federation/join-requests",
        None,
        Some("{}"),
        &[("Idempotency-Key", "chave com espaco")],
    );
    assert_eq!(res.status, 422);
    assert_eq!(res.json()["error"], "invalid_request");

    pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn capacidades_do_meta_refletem_o_processo() {
    let Some(_env) = env().await else { return };
    let addr = serve_com_banco().await;
    let m = meta(addr);
    let caps = &m["capabilities"];
    // Router de teste: sem runtime de malha, sem publicador, métricas montadas.
    assert_eq!(caps["federation_enabled"], false);
    assert_eq!(caps["outbox_enabled"], false);
    assert_eq!(caps["host_metrics_available"], true);
    let classes = caps["retention_classes"]
        .as_array()
        .unwrap_or_else(|| panic!("com banco vivo, retention_classes é conhecida: {caps}"));
    assert!(
        classes.iter().any(|c| c.as_str() == Some("incidente_sos")),
        "classe semeada não apareceu: {classes:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn contrato_publica_corpo_e_erros_das_decisoes_de_ingresso() {
    let addr = serve_sem_banco().await;
    let meta = meta(addr);
    let doc = openapi(addr);
    let errors = meta["errors"].as_array().unwrap();
    let codes = [
        "invalid_request",
        "approval_signature_invalid",
        "approver_not_authorized",
        "approval_not_yet_valid",
        "approval_expired",
        "approval_epoch_regressed",
        "no_authorized_approver",
        "federation_governance_disabled",
    ];
    for (action, domain) in [
        ("approve", "godmesh.federation.approval.v1"),
        ("reject", "godmesh.federation.rejection.v1"),
    ] {
        let path = format!("/v1/federation/join-requests/{{join_request_id}}/{action}");
        let op = &doc["paths"][path]["post"];
        assert!(op["summary"].as_str().unwrap().contains(domain));
        assert_eq!(
            op["requestBody"]["content"]["application/json"]["schema"]["$ref"],
            "#/components/schemas/ApprovalDecision"
        );
        let response_schema = &op["responses"]["200"]["content"]["application/json"]["schema"];
        let alternatives = response_schema["oneOf"].as_array().expect("decisão e dry_run");
        for name in ["JoinRequestView", "DryRunResponse"] {
            let reference = format!("#/components/schemas/{name}");
            assert!(alternatives.iter().any(|schema| schema["$ref"] == reference));
        }
        let response = &op["responses"]["422"];
        for code in codes {
            assert!(errors.iter().any(|e| e["error"] == code && e["http_status"] == 422));
            assert!(response["description"].as_str().unwrap().contains(code), "{action}: {code}");
        }
    }
}
