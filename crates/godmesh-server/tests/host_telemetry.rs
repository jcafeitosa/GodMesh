//! Telemetria de host no fio: `/metrics` com `godmesh_host_*` e o resumo de
//! capacidade do `/healthz` (ADR-073, camada 1 — observar).
//!
//! O que este arquivo prova:
//!  * o texto `godmesh_host_*` chega ao cliente **pela rota autenticada**, com
//!    escopo `admin:metrics`, e um scrape não abre `/proc` (o handler lê o
//!    snapshot publicado);
//!  * os números publicados são os da árvore lida pelo coletor — a mesma
//!    fixture do crate `godmesh-host`, para haver uma fonte única do formato
//!    de `/proc` neste repositório;
//!  * **pressão não julga saúde nesta fatia**: um snapshot com CPU a 100 % de
//!    stall continua respondendo 200, e o corpo diz isso explicitamente;
//!  * snapshot ausente ou velho é declarado como indisponível, não como zero.
//!
//! Banco próprio desta suíte:
//!   createdb godmesh_h1
//!   DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_h1 \
//!     cargo test -p godmesh-server --test host_telemetry -- --nocapture
//! Sem `DATABASE_URL`, os testes que precisam de esquema são pulados (não
//! fingem sucesso); os que só exercitam o exposition rodam sempre.

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use godmesh_host::collect::{Collector, SharedSnapshot, StaticVolumes};
use godmesh_host::read::Read as HostRead;
use godmesh_host::snapshot::{Group, Snapshot, Volume};
use godmesh_identity::grant::{FreshnessSource, GrantPredicate, OfflineOp, RevocationState};
use godmesh_server::auth::{Authorization, GrantRegistry, Scope, Subject, VerifiedGrant};
use godmesh_server::http::HostTelemetry;
use godmesh_server::observability::StderrRequestLog;
use sqlx::PgPool;

mod common;

static SEQ: AtomicU64 = AtomicU64::new(0);

fn unique(tag: &str) -> String {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    format!("{tag}-{}-{n}", std::process::id())
}

fn grant(tenant_id: &str, token_id: &str, scopes: Vec<Scope>) -> VerifiedGrant {
    VerifiedGrant {
        subject: Subject {
            tenant_id: tenant_id.to_owned(),
            principal: "principal-host".to_owned(),
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

/// Árvore de `/proc` de fixture, na fonte única do formato no repositório.
fn fixture_root(nome: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("godmesh-host")
        .join("tests")
        .join("fixtures")
        .join(nome);
    assert!(
        path.is_dir(),
        "fixture {nome} esperada em {}",
        path.display()
    );
    path
}

/// Coletor sobre uma **cópia temporária** da fixture, publicando num snapshot
/// compartilhado — o mesmo objeto que `main.rs` anexa ao router.
///
/// A cópia existe por um motivo concreto: a janela de taxa de CPU só nasce de
/// duas amostras com contadores diferentes, e para isso o `/proc/stat` lido
/// entre as coletas precisa mudar. Mudar a fixture do repositório afetaria os
/// testes do outro crate; então o teste copia, coleta, reescreve e coleta de
/// novo — exatamente o que o `GODMESH_HOST_PROC_ROOT` faz num nó real.
fn coletor_de_fixture(
    nome: &str,
    published: &SharedSnapshot,
    volumes: Vec<Volume>,
) -> Snapshot {
    let raiz = copia_temporaria(nome);
    let collector = Collector::with_read(HostRead::new(&raiz), Arc::new(StaticVolumes(volumes)));
    std::mem::drop(collector.collect());
    std::fs::write(
        raiz.join("proc/stat"),
        "cpu  1100 200 350 5400 420 10 20 60 0 0\ncpu0 1 0 0 0 0 0 0 0 0 0\n",
    )
    .expect("reescreve proc/stat na cópia");
    let snapshot = collector.collect();
    published.store(snapshot.clone());
    let _ = std::fs::remove_dir_all(&raiz);
    snapshot
}

/// Cópia recursiva de uma fixture para um diretório temporário, removida no
/// `Drop` do nome único.
fn copia_temporaria(nome: &str) -> PathBuf {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let destino = std::env::temp_dir().join(format!(
        "godmesh-server-{}-{nome}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    copiar(&fixture_root(nome), &destino);
    destino
}

fn copiar(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).expect("cria diretório de cópia");
    for entrada in std::fs::read_dir(de).expect("lê diretório de fixture") {
        let entrada = entrada.expect("entrada");
        let destino = para.join(entrada.file_name());
        if entrada.file_type().expect("tipo").is_dir() {
            copiar(&entrada.path(), &destino);
        } else {
            std::fs::copy(entrada.path(), destino).expect("copia arquivo de fixture");
        }
    }
}

fn volumes_de_teste() -> Vec<Volume> {
    vec![Volume {
        mount_point: "/".to_owned(),
        file_system: "ext4".to_owned(),
        total_bytes: 40 * 1024 * 1024 * 1024,
        available_bytes: 20 * 1024 * 1024 * 1024,
        read_only: false,
    }]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metrics_publica_host_pela_rota_autenticada() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let tenant = unique("tenant-host");
    sqlx::query(
        "INSERT INTO tenant_region (tenant_id, jurisdiction) VALUES ($1, $2) \
         ON CONFLICT (tenant_id) DO UPDATE SET jurisdiction = EXCLUDED.jurisdiction",
    )
    .bind(&tenant)
    .bind("BR")
    .execute(&pool)
    .await
    .expect("jurisdição de teste declarada");

    let admin_token = unique("grant-host-admin");
    let read_token = unique("grant-host-read");
    let published = SharedSnapshot::new();
    let snapshot = coletor_de_fixture("linux", &published, volumes_de_teste());

    let addr = serve(
        pool.clone(),
        vec![
            (
                admin_token.clone(),
                grant(&tenant, "host-admin", vec![Scope::AdminMetrics]),
            ),
            (
                read_token.clone(),
                grant(&tenant, "host-read", vec![Scope::IncidentRead]),
            ),
        ],
        HostTelemetry::new(published.clone(), Duration::from_secs(15)),
    )
    .await;

    let (status, _, body) = http(addr, "/metrics", None);
    assert_eq!(status, 401, "sem credencial: {body}");
    let (status, _, body) = http(addr, "/metrics", Some(&format!("Bearer {read_token}")));
    assert_eq!(status, 403, "sem escopo: {body}");

    let (status, content_type, body) =
        http(addr, "/metrics", Some(&format!("Bearer {admin_token}")));
    assert_eq!(status, 200, "{body}");
    assert!(content_type.starts_with("text/plain"), "{content_type}");

    // as métricas de aplicação continuam lá
    for trecho in [
        "godmesh_http_requests_total{route=\"/metrics\",status=\"401\"} ",
        "godmesh_pool_connections{state=\"size\"} ",
    ] {
        assert!(body.contains(trecho), "faltou {trecho:?}");
    }

    // e a seção de host chegou com os números da árvore lida
    for trecho in [
        "godmesh_host_metrics_available{source=\"proc\"} 1",
        "godmesh_host_memory_bytes{state=\"total\"} 16777216000",
        "godmesh_host_load_average{window=\"1m\"} 0.420000",
        "godmesh_host_cpu_seconds_total{class=\"io_wait\"} 4",
        "godmesh_host_tcp_connections{state=\"time_wait\"} 1",
        "godmesh_host_network_interface_bytes_total{interface=\"eth0\",direction=\"rx\"} 5000000",
        "godmesh_host_conntrack_entries 1234",
        "godmesh_host_conntrack_limit 65536",
        "godmesh_host_process_descriptors{state=\"open\"} 8",
        "godmesh_host_process_threads 9",
        "godmesh_host_disk_bytes{volume=\"/\",state=\"total\"} 42949672960",
        "godmesh_host_group_available{group=\"cgroup\"} 0",
    ] {
        assert!(body.contains(trecho), "faltou {trecho:?} em:\n{body}");
    }

    // a taxa de CPU existe porque houve duas coletas
    assert!(
        snapshot.cpu_window.is_some(),
        "janela de taxa deveria existir na segunda coleta"
    );
    assert!(
        body.contains("godmesh_host_cpu_usage_ratio{class=\"busy\"}"),
        "faltou a taxa de CPU:\n{body}"
    );

    // cardinalidade: a seção de host é contada, e o teto é o que o teste
    // verifica — se alguém colocar uma série por conexão, isso estoura aqui.
    let linhas_de_host = body
        .lines()
        .filter(|line| line.starts_with("godmesh_host_"))
        .count();
    let series_de_aplicacao = body
        .lines()
        .filter(|line| line.starts_with("godmesh_") && !line.starts_with("godmesh_host_"))
        .count();
    println!(
        "scrape: {} linhas totais | antes (aplicação): {} séries | agora (host): {} séries",
        body.lines().count(),
        series_de_aplicacao,
        linhas_de_host
    );
    assert!(
        linhas_de_host <= 200,
        "seção de host com {linhas_de_host} séries: acima do teto declarado"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthz_da_capacidade_sem_julgar_pressao() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let published = SharedSnapshot::new();
    // fixture `container`: pressão de CPU 30 % no cgroup e 12,5 % no host
    coletor_de_fixture("container", &published, volumes_de_teste());

    let addr = serve(pool.clone(), vec![], HostTelemetry::new(published, Duration::from_secs(15))).await;
    let (status, _, body) = http(addr, "/healthz", None);

    // Pressão alta NÃO é falha nesta fatia: readiness por pressão é a camada 2
    // e ainda não tem limiar nem histerese aprovados.
    assert_eq!(status, 200, "pressão não derruba o healthz; corpo: {body}");
    assert!(body.contains("\"status\":\"ok\""), "{body}");
    assert!(body.contains("\"capacity\""), "faltou o resumo: {body}");
    assert!(body.contains("\"cpu_pressure\":30"), "corpo: {body}");
    assert!(body.contains("\"memory_pressure\":25"), "corpo: {body}");
    assert!(body.contains("\"disk_free_ratio\":0.5"), "corpo: {body}");
    assert!(
        body.contains("\"pressure_is_not_a_health_signal\":true"),
        "o corpo tem que dizer que pressão não julga: {body}"
    );
    assert!(body.contains("\"container\":true"), "corpo: {body}");
    assert!(body.contains("\"source\":\"proc\""), "corpo: {body}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthz_sem_coletor_diz_null_e_continua_200() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let addr = serve(pool.clone(), vec![], HostTelemetry::default()).await;
    let (status, _, body) = http(addr, "/healthz", None);
    assert_eq!(status, 200, "{body}");
    for trecho in [
        "\"cpu_pressure\":null",
        "\"memory_pressure\":null",
        "\"disk_free_ratio\":null",
        "\"source\":\"none\"",
        "\"snapshot_age_seconds\":null",
    ] {
        assert!(body.contains(trecho), "ausência declarada como null: {trecho} em {body}");
    }
}

#[test]
fn metrics_section_de_snapshot_velho_e_declarado_indisponivel() {
    // Sem banco: o comportamento do exposition é uma função pura do snapshot.
    let published = SharedSnapshot::new();
    let mut collector = Collector::with_read(
        HostRead::new(fixture_root("linux")),
        Arc::new(StaticVolumes(volumes_de_teste())),
    );
    let _ = &mut collector;
    let snapshot = collector.collect();
    let mut velho = snapshot.clone();
    velho.taken_at_millis = 1; // 1970: o mais velho possível
    published.store(velho);

    let telemetry = HostTelemetry::new(published, Duration::from_secs(15));
    let texto = telemetry.metrics_section();
    assert!(
        texto.contains("godmesh_host_metrics_available{source=\"none\"} 0"),
        "snapshot velho não é servido como atual:\n{texto}"
    );
    assert!(!texto.contains("godmesh_host_memory_bytes"), "{texto}");
}

#[test]
fn sem_snapshot_a_secao_existe_e_declara_a_falta() {
    let telemetry = HostTelemetry::default();
    let texto = telemetry.metrics_section();
    assert!(
        texto.contains("godmesh_host_metrics_available{source=\"none\"} 0"),
        "{texto}"
    );
    assert!(
        texto.contains("godmesh_host_group_available{group=\"cpu\"} 0"),
        "{texto}"
    );
    // e nada de número inventado
    for proibido in [
        "godmesh_host_memory_bytes",
        "godmesh_host_cpu_seconds_total",
        "godmesh_host_disk_bytes",
        "godmesh_host_tcp_connections",
    ] {
        assert!(!texto.contains(proibido), "{proibido} não pode sair sem leitura:\n{texto}");
    }
}

#[test]
fn capacity_json_diz_origem_e_nao_aprova_nada() {
    let published = SharedSnapshot::new();
    let collector = Collector::with_read(
        HostRead::new(fixture_root("container")),
        Arc::new(StaticVolumes(volumes_de_teste())),
    );
    let snapshot = collector.collect();
    published.store(snapshot);

    let capacidade = HostTelemetry::new(published, Duration::from_secs(15)).capacity_json();
    assert_eq!(capacidade["cpu_pressure"], serde_json::json!(30.0));
    assert_eq!(capacidade["source"], serde_json::json!("proc"));
    assert_eq!(
        capacidade["pressure_is_not_a_health_signal"],
        serde_json::json!(true)
    );
    // Em contêiner o resumo é o número do namespace, não o do host.
    let bruto = Snapshot::empty();
    assert_eq!(bruto.capacity().cpu_pressure, None);
}

#[test]
fn grupos_de_fixture_linux_estao_todos_presentes_menos_cgroup() {
    let collector = Collector::with_read(
        HostRead::new(fixture_root("linux")),
        Arc::new(StaticVolumes(volumes_de_teste())),
    );
    let snapshot = collector.collect();
    for grupo in Group::ALL {
        if grupo == Group::Cgroup {
            assert!(!snapshot.group_available(grupo), "{grupo:?} sem namespace");
        } else {
            assert!(snapshot.group_available(grupo), "{grupo:?} deveria estar lido");
        }
    }
}

// --- infraestrutura de teste ------------------------------------------------

async fn serve(
    pool: PgPool,
    entries: Vec<(String, VerifiedGrant)>,
    host: HostTelemetry,
) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let authorization = Authorization::new(GrantRegistry::from_entries(entries), pool.clone());
    let app = godmesh_server::http::router_with_host(
        pool,
        authorization,
        None,
        host,
    );
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
        .unwrap_or_else(|| panic!("resposta sem headers: {raw:?}"));
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

/// O router de teste usa `StderrRequestLog` por padrão; nada aqui depende de
/// inspecionar o log.
#[allow(dead_code)]
fn log_padrao() -> Arc<StderrRequestLog> {
    Arc::new(StderrRequestLog)
}

#[allow(dead_code)]
fn caminho_de_fixture(nome: &str) -> PathBuf {
    fixture_root(nome)
}
