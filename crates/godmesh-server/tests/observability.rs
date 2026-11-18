//! `GET /metrics` e log estruturado de requisição, no fio.
//!
//! Prova três coisas observáveis:
//!  * `/metrics` exige credencial **e** escopo `admin:metrics` (401/403), e o
//!    texto publica contadores por rota+status, histograma de latência e gauges
//!    de pool/outbox depois de requisições reais;
//!  * cada requisição gera **uma** linha com `request_id`, rota, método, status,
//!    duração, `token_id` e tenant;
//!  * o token apresentado nunca aparece em nenhuma linha.
//!
//! Banco próprio desta suíte, migrado pelo runner oficial (`tests/common/mod.rs`):
//!   createdb godmesh_t2
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_t2 \
//!     cargo test -p godmesh-server --test observability -- --nocapture
//!
//! Sem `DATABASE_URL` os testes são ignorados (não fingem sucesso).

mod common;

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{Authorization, GrantRegistry, Scope, Subject, VerifiedGrant};
use godmesh_server::observability::CapturedLog;
use sqlx::PgPool;

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{tag}-{}-{n}", std::process::id())
}

fn grant(tenant_id: &str, token_id: &str, scopes: Vec<Scope>) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant_id.to_owned(),
            principal: "principal-obs".to_owned(),
        },
        predicate: GrantPredicate {
            operation: OfflineOp::RecordLocalObservation,
            epoch_comparable: true,
            revocation: RevocationState::Absent,
            freshness: FreshnessSource::F3VerifiedOnline,
            retention_class_known: true,
            action_enumerated: true,
        },
        token_id: token_id.to_owned(),
        scopes,
        authority_epoch: 1,
    }
}

async fn pool() -> Option<PgPool> {
    common::pool_com_conexoes(8).await
}

async fn declare_tenant(pool: &PgPool, tenant: &str, jurisdiction: &str) {
    sqlx::query("INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, $2) ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction")
        .bind(tenant)
        .bind(jurisdiction)
        .execute(pool)
        .await
        .expect("jurisdição de teste declarada");
}

/// Servidor com log em memória, para inspecionar exatamente o que foi registrado.
async fn serve(
    pool: PgPool,
    entries: Vec<(String, VerifiedGrant)>,
    log: Arc<CapturedLog>,
) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let authorization = Authorization::new(GrantRegistry::from_entries(entries), pool.clone());
    let app = godmesh_server::http::router_with_log(pool, authorization, log);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn http(addr: SocketAddr, path: &str, authorization: Option<&str>) -> (u16, String, String) {
    let mut stream = std::net::TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("timeout de leitura");
    stream
        .set_write_timeout(Some(Duration::from_secs(15)))
        .expect("timeout de escrita");
    let mut request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n");
    if let Some(value) = authorization {
        request.push_str(&format!("Authorization: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).expect("envia pedido");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("lê resposta");
    let status: u16 = raw
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {raw:?}"));
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("resposta sem separador de headers: {raw:?}"));
    let mut content_type = String::new();
    for line in head.lines() {
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-type")
        {
            content_type = value.trim().to_owned();
        }
    }
    (status, content_type, body.to_owned())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_exige_escopo_admin_e_publica_contadores() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-obs");
    declare_tenant(&pool, &tenant, "BR").await;

    let read_token = unique("grant-obs-read");
    let admin_token = unique("grant-obs-admin");
    let log = Arc::new(CapturedLog::new());
    let addr = serve(
        pool.clone(),
        vec![
            (
                read_token.clone(),
                grant(&tenant, "obs-read", vec![Scope::IncidentRead]),
            ),
            (
                admin_token.clone(),
                grant(&tenant, "obs-admin", vec![Scope::AdminMetrics]),
            ),
        ],
        Arc::clone(&log),
    )
    .await;

    // Uma requisição real de leitura, para o contador existir.
    let (status, _, body) = http(
        addr,
        "/v1/incidents?limit=1",
        Some(&format!("Bearer {read_token}")),
    );
    assert_eq!(status, 200, "leitura autorizada: {body}");

    // Sem credencial: 401, nada de métrica.
    let (status, _, body) = http(addr, "/metrics", None);
    assert_eq!(status, 401, "{body}");

    // Credencial válida sem o escopo: 403 nomeado.
    let (status, _, body) = http(addr, "/metrics", Some(&format!("Bearer {read_token}")));
    assert_eq!(status, 403, "sem admin:metrics precisa ser 403: {body}");
    assert!(body.contains("insufficient_scope"), "{body}");

    // Credencial com o escopo: 200 em texto Prometheus.
    let (status, content_type, body) =
        http(addr, "/metrics", Some(&format!("Bearer {admin_token}")));
    assert_eq!(status, 200, "{body}");
    assert!(
        content_type.starts_with("text/plain"),
        "content-type precisa ser texto Prometheus: {content_type:?}"
    );

    let esperado = [
        // contadores por rota+status, acumulados inclusive os 401/403 acima
        "godmesh_http_requests_total{route=\"/v1/incidents\",status=\"200\"} ",
        "godmesh_http_requests_total{route=\"/metrics\",status=\"401\"} ",
        "godmesh_http_requests_total{route=\"/metrics\",status=\"403\"} ",
        // histograma de latência da rota
        "godmesh_http_request_duration_seconds_bucket{route=\"/v1/incidents\",le=\"+Inf\"} ",
        "godmesh_http_request_duration_seconds_count{route=\"/v1/incidents\"} ",
        "godmesh_http_request_duration_seconds_sum{route=\"/v1/incidents\"} ",
        // gauges
        "godmesh_pool_connections{state=\"size\"} ",
        "godmesh_pool_connections{state=\"idle\"} ",
        "godmesh_outbox_pending ",
        "godmesh_metrics_scrape_dependency_up{dependency=\"postgres\"} 1",
    ];
    for trecho in esperado {
        assert!(body.contains(trecho), "faltou {trecho:?} em:\n{body}");
    }

    // Onde há contador, o número é >= 1 (não é série vazia).
    let linha = body
        .lines()
        .find(|l| {
            l.starts_with("godmesh_http_requests_total{route=\"/v1/incidents\",status=\"200\"}")
        })
        .expect("contador da rota de incidentes");
    let valor: u64 = linha
        .rsplit(' ')
        .next()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("contador sem número: {linha:?}"));
    assert!(
        valor >= 1,
        "contador precisa contar a requisição feita: {linha:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_registra_identidade_e_nunca_o_token() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-obs-log");
    declare_tenant(&pool, &tenant, "BR").await;

    let read_token = unique("grant-obs-log-read");
    let log = Arc::new(CapturedLog::new());
    let addr = serve(
        pool.clone(),
        vec![(
            read_token.clone(),
            grant(&tenant, "obs-log-read", vec![Scope::IncidentRead]),
        )],
        Arc::clone(&log),
    )
    .await;

    let (status, _, body) = http(
        addr,
        "/v1/incidents?limit=1",
        Some(&format!("Bearer {read_token}")),
    );
    assert_eq!(status, 200, "{body}");

    // Requisição sem credencial: a linha existe e declara ausência de identidade.
    let (status, _, _) = http(addr, "/v1/incidents?limit=1", None);
    assert_eq!(status, 401);

    let lines = log.lines();
    assert!(!lines.is_empty(), "toda requisição precisa gerar linha");

    let linha_200 = lines
        .iter()
        .find(|l| l.contains("status=200") && l.contains("route=/v1/incidents"))
        .unwrap_or_else(|| panic!("linha da requisição 200 ausente em {lines:?}"));
    for campo in [
        "request_id=req-",
        "method=GET",
        "route=/v1/incidents",
        "status=200",
        "duration_ms=",
        "token_id=obs-log-read",
    ] {
        assert!(
            linha_200.contains(campo),
            "faltou {campo:?} em {linha_200:?}"
        );
    }
    assert!(
        linha_200.contains(&format!("tenant={tenant}")),
        "tenant precisa vir do grant verificado: {linha_200:?}"
    );
    // O caminho cru (com eventual id) não entra no log: só o padrão da rota.
    assert!(
        !linha_200.contains("limit="),
        "query não pertence ao log: {linha_200:?}"
    );

    let linha_401 = lines
        .iter()
        .find(|l| l.contains("status=401") && l.contains("route=/v1/incidents"))
        .unwrap_or_else(|| panic!("linha da requisição 401 ausente em {lines:?}"));
    assert!(
        linha_401.contains("token_id=-") && linha_401.contains("tenant=-"),
        "ausência de identidade é registrada como ausência: {linha_401:?}"
    );

    // Nenhuma linha contém o token apresentado — em nenhum campo.
    for line in &lines {
        assert!(
            !line.contains(&read_token),
            "token não pode aparecer no log: {line:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn log_e_metrica_cobrem_rota_nao_casada() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-obs-404");
    declare_tenant(&pool, &tenant, "BR").await;
    let admin_token = unique("grant-obs-404-admin");
    let log = Arc::new(CapturedLog::new());
    let addr = serve(
        pool.clone(),
        vec![(
            admin_token.clone(),
            grant(&tenant, "obs-404-admin", vec![Scope::AdminMetrics]),
        )],
        Arc::clone(&log),
    )
    .await;

    let (status, _, _) = http(
        addr,
        "/v1/nao-existe",
        Some(&format!("Bearer {admin_token}")),
    );
    assert_eq!(status, 404, "rota inexistente responde 404 do router");

    let lines = log.lines();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("route=unmatched") && l.contains("status=404")),
        "rota não casada precisa aparecer como unmatched, não como caminho cru: {lines:?}"
    );

    let (status, _, body) = http(addr, "/metrics", Some(&format!("Bearer {admin_token}")));
    assert_eq!(status, 200, "{body}");
    assert!(
        body.contains("godmesh_http_requests_total{route=\"unmatched\",status=\"404\"} "),
        "contador de rota não casada ausente:\n{body}"
    );
}
