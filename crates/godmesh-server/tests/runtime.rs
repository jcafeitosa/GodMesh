//! Ciclo de vida local do servidor: migrações, `/healthz` e shutdown.
//!
//! Banco próprio: `godmesh_sa` (nome configurável por `GODMESH_SA_TEST_DB`),
//! **criado pelo próprio binário** e migrado pelo binário (`--migrate-only`).
//! O banco de desenvolvimento (`godmesh`) nunca é tocado aqui.
//!
//! Conexão base: `DATABASE_URL` (ex.:
//! `postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh`). Só a parte do
//! banco é substituída. Sem `DATABASE_URL` os testes são ignorados — não fingem
//! sucesso.
//!
//! Como o banco derrubado é simulado (`/healthz` 503): `ALTER DATABASE godmesh_sa
//! ALLOW_CONNECTIONS false` + `pg_terminate_backend` nas sessões abertas naquele
//! banco, pelo pool de administração. Não derruba contêiner nem afeta outro
//! banco, e é reversível — a recuperação (`200` de novo) também é provada.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use godmesh_server::auth::GrantRegistry;
use godmesh_server::{http, migrate, runtime};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Nome do banco dos testes desta fatia.
const SA_DEFAULT: &str = "godmesh_sa";

/// Serializa os testes que mexem no banco `godmesh_sa`. `cargo test` roda os
/// testes de um binário em paralelo; sem o cadeado, um derrubaria a conexão do
/// outro no meio da prova.
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn sole_lock() -> tokio::sync::MutexGuard<'static, ()> {
    DB_LOCK.lock().await
}

fn base_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

fn sa_db() -> String {
    std::env::var("GODMESH_SA_TEST_DB").unwrap_or_else(|_| SA_DEFAULT.to_owned())
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

fn sa_url(base: &str) -> String {
    with_database(base, &sa_db())
}

/// Caminho de identidade de nó; deixado inexistente para o binário criar com 0600.
fn caminho_chave_no() -> PathBuf {
    let caminho = std::env::temp_dir().join(format!("godmesh-rt-node-{}.key", std::process::id()));
    let _ = std::fs::remove_file(&caminho);
    caminho
}

fn binario() -> &'static str {
    env!("CARGO_BIN_EXE_godmesh-server")
}

/// Diretório de migrações do repositório, resolvido pelo próprio manifesto.
fn migrations_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    dir.canonicalize()
        .expect("diretório migrations do repositório")
}

/// Nomes `*.sql` do diretório, em ordem — a mesma ordem do runner.
fn nomes_de_migracao(dir: &Path) -> Vec<String> {
    let mut nomes: Vec<String> = std::fs::read_dir(dir)
        .expect("lê migrations")
        .filter_map(|e| {
            let path = e.ok()?.path();
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("sql") {
                return None;
            }
            path.file_name()?.to_str().map(str::to_owned)
        })
        .collect();
    nomes.sort();
    nomes
}

async fn admin_pool(base: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .connect(&with_database(base, "postgres"))
        .await
        .expect("conexão de administração ao Postgres")
}

// `AssertSqlSafe` nos DDL de banco de teste: o nome do banco vem de
// `GODMESH_SA_TEST_DB`/constante deste arquivo, nunca de rede, e identificador
// não pode ser parâmetro de bind em `CREATE`/`DROP`/`ALTER DATABASE`.

/// Recria `godmesh_sa` do zero. `FORCE` derruba sessões remanescentes do teste
/// anterior — é banco descartável, criado por este binário de teste.
async fn recreate_sa(admin: &PgPool) {
    let db = sa_db();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE IF EXISTS {db} WITH (FORCE)"
    )))
    .execute(admin)
    .await
    .expect("drop do banco de teste");
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {db}")))
        .execute(admin)
        .await
        .expect("create do banco de teste");
}

/// Cria `godmesh_sa` se ainda não existir.
async fn ensure_sa(admin: &PgPool) {
    let db = sa_db();
    let existe: Option<String> =
        sqlx::query_scalar("SELECT datname FROM pg_database WHERE datname = $1")
            .bind(&db)
            .fetch_optional(admin)
            .await
            .expect("consulta pg_database");
    if existe.is_none() {
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {db}")))
            .execute(admin)
            .await
            .expect("create do banco de teste");
    }
}

fn run_migrate_only(url: &str, dir: &Path) -> Output {
    Command::new(binario())
        .arg("--migrate-only")
        .env("DATABASE_URL", url)
        .env("GODMESH_MIGRATIONS_DIR", dir)
        .env_remove("GODMESH_GRANTS")
        .env_remove("BIND_ADDR")
        .output()
        .expect("binário de teste executa")
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Linhas do controle: nome, checksum e instante de aplicação em texto (sem
/// depender de feature de data do sqlx).
async fn control_rows(pool: &PgPool) -> Vec<(String, String, String)> {
    sqlx::query_as("SELECT nome, checksum, aplicado_em::text FROM schema_migration ORDER BY nome")
        .fetch_all(pool)
        .await
        .expect("leitura de schema_migration")
}

/// GET HTTP/1.1 cru: prova no fio, não só a função de handler.
fn http_get(addr: SocketAddr, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("conecta ao servidor de teste");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout de leitura");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .expect("envia o pedido");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("lê a resposta");
    let status: u16 = raw
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("linha de status HTTP");
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();
    (status, body)
}

/// Sobe o router real em porta efêmera, sem shutdown gracioso: estes testes
/// olham `/healthz`, não o encerramento.
async fn serve_router(app: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    addr
}

fn empty_grants() -> GrantRegistry {
    GrantRegistry::from_json("{\"grants\":[]}").expect("registro vazio é válido")
}

async fn pool_for(url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(url)
        .await
        .expect("pool do banco de teste")
}

// --- 1. migração na inicialização ---------------------------------------------

#[tokio::test]
async fn migracao_roda_duas_vezes_sem_duplicar_e_sem_reescrever_aplicado_em() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;
    let url = sa_url(&base);
    let dir = migrations_dir();

    let primeira = run_migrate_only(&url, &dir);
    assert!(
        primeira.status.success(),
        "1ª execução falhou: {}\n{}",
        primeira.status,
        stderr_of(&primeira)
    );

    let pool = pool_for(&url).await;
    let depois_da_primeira = control_rows(&pool).await;
    let arquivos = nomes_de_migracao(&dir);
    assert_eq!(
        depois_da_primeira
            .iter()
            .map(|(nome, _, _)| nome.clone())
            .collect::<Vec<_>>(),
        arquivos,
        "o controle deve listar exatamente as migrações do diretório, em ordem"
    );
    for (nome, checksum, _) in &depois_da_primeira {
        assert_eq!(checksum.len(), 64, "{nome}: checksum sha256 em hex");
    }

    let segunda = run_migrate_only(&url, &dir);
    assert!(
        segunda.status.success(),
        "2ª execução (idempotente) falhou: {}\n{}",
        segunda.status,
        stderr_of(&segunda)
    );
    let saida = stdout_of(&segunda);
    assert!(
        saida.contains("0 aplicada(s)") && saida.contains("já aplicada(s)"),
        "2ª execução deveria relatar tudo como já aplicado:\n{saida}"
    );

    let depois_da_segunda = control_rows(&pool).await;
    assert_eq!(
        depois_da_primeira, depois_da_segunda,
        "rodar de novo não pode duplicar linha nem reescrever aplicado_em"
    );
    pool.close().await;
}

#[tokio::test]
async fn migracao_com_checksum_divergente_falha_alto() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;
    let url = sa_url(&base);
    let dir = migrations_dir();

    assert!(
        run_migrate_only(&url, &dir).status.success(),
        "migração inicial"
    );

    let pool = pool_for(&url).await;
    let nome = nomes_de_migracao(&dir)
        .into_iter()
        .next()
        .expect("ao menos uma migração");
    sqlx::query("UPDATE schema_migration SET checksum = $1 WHERE nome = $2")
        .bind("0".repeat(64))
        .bind(&nome)
        .execute(&pool)
        .await
        .expect("adultera o checksum registrado");

    let tentativa = run_migrate_only(&url, &dir);
    assert!(
        !tentativa.status.success(),
        "checksum divergente precisa falhar o processo, não passar em silêncio:\n{}",
        stdout_of(&tentativa)
    );
    let erro = stderr_of(&tentativa);
    assert!(
        erro.contains(&nome) && erro.contains("checksum"),
        "erro precisa nomear a migração e o checksum:\n{erro}"
    );

    // O histórico não é "consertado" pelo runner: nada além da adulteração mudou.
    let linhas = control_rows(&pool).await;
    assert_eq!(linhas.len(), nomes_de_migracao(&dir).len());
    pool.close().await;
}

#[tokio::test]
async fn migracao_quebrada_falha_alto_e_nao_grava_controle() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;
    let url = sa_url(&base);
    let dir = migrations_dir();

    assert!(
        run_migrate_only(&url, &dir).status.success(),
        "migração inicial"
    );
    let pool = pool_for(&url).await;
    let antes = control_rows(&pool).await;

    // Diretório alternativo: prova também que GODMESH_MIGRATIONS_DIR é honrado.
    let temp = std::env::temp_dir().join(format!(
        "godmesh-mig-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("relógio do sistema")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp).expect("diretório temporário de migrações");
    std::fs::write(
        temp.join("9999_quebrada.sql"),
        "CREATE TABLE tabela_que_nao_deve_existir (id int NOT NULL PRIMARY KEY);\n\
         CREATE TABLE tabela_que_nao_deve_existir (id int);\n",
    )
    .expect("escreve migração quebrada");

    let tentativa = run_migrate_only(&url, &temp);
    let _ = std::fs::remove_dir_all(&temp);
    assert!(
        !tentativa.status.success(),
        "migração que quebra precisa derrubar o processo:\n{}",
        stdout_of(&tentativa)
    );
    let erro = stderr_of(&tentativa);
    assert!(
        erro.contains("9999_quebrada.sql"),
        "erro precisa nomear o arquivo:\n{erro}"
    );

    // Bloco transacional implícito: nem a tabela do script nem a linha de
    // controle sobreviveram à falha.
    let tabela: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.tabela_que_nao_deve_existir')::text")
            .fetch_one(&pool)
            .await
            .expect("consulta to_regclass");
    assert!(
        tabela.is_none(),
        "migração que falhou deixou objeto no banco"
    );
    assert_eq!(
        control_rows(&pool).await,
        antes,
        "controle mudou após falha"
    );
    pool.close().await;
}

// --- 2. /healthz com dependência real -----------------------------------------

// Multi-thread de propósito: o cliente HTTP abaixo é bloqueante e precisa de
// outra worker para o servidor em processo responder.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn healthz_e_200_com_banco_e_503_com_banco_derrubado_e_volta() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;
    let url = sa_url(&base);
    let dir = migrations_dir();
    let migrou = run_migrate_only(&url, &dir);
    assert!(migrou.status.success(), "{}", stderr_of(&migrou));

    let pool = pool_for(&url).await;
    let addr = serve_router(http::router(pool.clone(), empty_grants())).await;

    let (status, body) = http_get(addr, "/healthz");
    assert_eq!(status, 200, "banco ok precisa dar 200; corpo: {body}");
    assert!(body.contains("\"status\":\"ok\""), "corpo: {body}");
    assert!(
        body.contains("\"ack_class\":\"transport_accepted\""),
        "ack_class sai nas duas respostas; corpo: {body}"
    );

    // Banco indisponível: fecha a porta do banco e derruba as sessões abertas.
    let db = sa_db();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER DATABASE {db} ALLOW_CONNECTIONS false"
    )))
    .execute(&admin)
    .await
    .expect("desabilita conexões ao banco de teste");
    let terminadas: i64 =
        sqlx::query_scalar("SELECT count(*) FROM (SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1) AS t")
            .bind(&db)
            .fetch_one(&admin)
            .await
            .expect("termina sessões no banco de teste");
    assert!(terminadas >= 0, "contagem de sessões terminadas");

    let (status, body) = http_get(addr, "/healthz");
    // Restaura antes de qualquer asserção que possa abortar o teste.
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "ALTER DATABASE {db} ALLOW_CONNECTIONS true"
    )))
    .execute(&admin)
    .await
    .expect("reabilita conexões ao banco de teste");

    assert_eq!(status, 503, "banco fora precisa dar 503; corpo: {body}");
    assert!(
        body.contains("\"status\":\"unavailable\""),
        "corpo precisa declarar indisponibilidade: {body}"
    );
    assert!(
        body.contains("\"dependency\":\"postgres\""),
        "corpo precisa nomear a dependência: {body}"
    );
    assert!(
        body.contains("\"ack_class\":\"transport_accepted\""),
        "ack_class se mantém no 503; corpo: {body}"
    );

    // Recuperação: o mesmo processo volta a responder 200 sem reiniciar.
    let mut ultimo = (0u16, String::new());
    for _ in 0..50 {
        ultimo = http_get(addr, "/healthz");
        if ultimo.0 == 200 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        ultimo.0, 200,
        "não voltou a 200 após religar o banco: {}",
        ultimo.1
    );
    pool.close().await;
}

// --- 3. shutdown gracioso -----------------------------------------------------

/// O `serve` do binário: requisição em voo termina depois do sinal, e o pool
/// fecha. Prova a drenagem, que um `drop` de runtime não faria.
#[tokio::test]
async fn shutdown_drena_requisicao_em_voo_e_fecha_o_pool() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    ensure_sa(&admin).await;
    let pool = pool_for(&sa_url(&base)).await;

    // Rota lenta: mantém a requisição em voo enquanto o sinal chega.
    let app = Router::new().route(
        "/slow",
        get(|| async {
            tokio::time::sleep(Duration::from_millis(400)).await;
            "drenada"
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind efêmero");
    let addr = listener.local_addr().expect("endereço local");
    let (sinal, espera) = tokio::sync::oneshot::channel::<()>();

    let servidor = tokio::spawn(runtime::serve(listener, app, pool.clone(), async move {
        let _ = espera.await;
    }));

    // Cliente começa a requisição antes do sinal e só termina depois dele.
    let cliente = tokio::task::spawn_blocking(move || http_get(addr, "/slow"));
    tokio::time::sleep(Duration::from_millis(150)).await;
    sinal.send(()).expect("envia o sinal de shutdown");

    let relatorio = servidor
        .await
        .expect("tarefa do servidor")
        .expect("serve sem erro de I/O");
    assert_eq!(
        relatorio.drained, 1,
        "a requisição em voo no instante do sinal precisa ser contada como drenada"
    );

    let (status, body) = cliente.await.expect("cliente HTTP");
    assert_eq!(
        status, 200,
        "requisição em voo precisa terminar com sucesso"
    );
    assert_eq!(body, "drenada");

    // Pool fechado: nenhuma conexão nova, e a consulta passa a falhar.
    let erro = sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .expect_err("pool precisa estar fechado depois do shutdown");
    assert!(
        erro.to_string().contains("closed"),
        "erro deveria dizer que o pool está fechado: {erro}"
    );
}

/// O binário de verdade: SIGTERM encerra com código 0 e imprime a contagem
/// drenada. Sem isso, o sinal só existiria no teste em processo.
#[cfg(unix)]
#[tokio::test]
async fn binario_encerra_por_sigterm_com_codigo_zero() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let admin = admin_pool(&base).await;
    recreate_sa(&admin).await;

    let mut filho = Command::new(binario())
        .env("DATABASE_URL", sa_url(&base))
        .env("GODMESH_MIGRATIONS_DIR", migrations_dir())
        .env("GODMESH_GRANTS", "{\"grants\":[]}")
        .env("GODMESH_NODE_KEY", caminho_chave_no())
        .env("GODMESH_ENV", "dev")
        .env("BIND_ADDR", "127.0.0.1:0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sobe o binário de teste");

    let mut log = String::new();
    let mut linhas = BufReader::new(filho.stdout.take().expect("stdout")).lines();
    let mut escutando = false;
    for _ in 0..50 {
        match linhas.next() {
            Some(Ok(linha)) => {
                log.push_str(&linha);
                log.push('\n');
                if linha.contains("escutando em") {
                    escutando = true;
                    break;
                }
            }
            Some(Err(e)) => panic!("stdout ilegível: {e}"),
            None => break,
        }
    }
    assert!(escutando, "o binário não chegou a escutar:\n{log}");

    let pid = filho.id();
    let sinal = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("envia SIGTERM");
    assert!(sinal.success(), "kill -TERM falhou: {sinal}");

    let mut codigo = None;
    for _ in 0..100 {
        if let Some(status) = filho.try_wait().expect("try_wait") {
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
    assert_eq!(codigo, Some(0), "SIGTERM precisa sair com 0; log:\n{log}");
    assert!(
        log.contains("em voo drenada"),
        "log precisa trazer a contagem drenada:\n{log}"
    );
    assert!(
        log.contains("encerrado"),
        "log precisa registrar o encerramento:\n{log}"
    );
}

/// O runner é o dono da transação: nenhum arquivo de migração abre `BEGIN`.
#[test]
fn migracoes_nao_abrem_transacao_propria() {
    for nome in nomes_de_migracao(&migrations_dir()) {
        let sql = std::fs::read_to_string(migrations_dir().join(&nome)).expect("lê migração");
        for (indice, linha) in sql.lines().enumerate() {
            let sem_espaco = linha.trim().to_ascii_lowercase();
            assert!(
                !(sem_espaco == "begin;" || sem_espaco == "commit;"),
                "{nome}:{}: o runner é dono da transação; remova {linha:?}",
                indice + 1
            );
        }
    }
}

/// O esquema de controle é o documentado (`schema_migration` v1).
#[test]
fn esquema_de_migracao_e_v1() {
    assert_eq!(migrate::SCHEMA_VERSION, "schema_migration v1");
    assert_eq!(migrate::DEFAULT_MIGRATIONS_DIR, "migrations");
    assert_eq!(migrate::MIGRATIONS_DIR_ENV, "GODMESH_MIGRATIONS_DIR");
}

async fn schema_without(base: &str, excluded: &[&str]) -> (String, PgPool) {
    recreate_sa(&admin_pool(base).await).await;
    let url = sa_url(base);
    let dir = migrations_dir();
    let old = std::env::temp_dir().join(format!("godmesh-m1bi-{}", std::process::id()));
    std::fs::create_dir(&old).unwrap();
    for name in nomes_de_migracao(&dir) {
        if !excluded.iter().any(|prefix| name.starts_with(prefix)) {
            std::fs::copy(dir.join(&name), old.join(&name)).unwrap();
        }
    }
    let migration = run_migrate_only(&url, &old);
    std::fs::remove_dir_all(old).unwrap();
    assert!(migration.status.success(), "{}", stderr_of(&migration));
    let pool = pool_for(&url).await;
    (url, pool)
}

#[tokio::test]
async fn m1bi_applies_0009_after_0010_once() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let (url, pool) = schema_without(&base, &["0009", "0011"]).await;
    sqlx::raw_sql(
        "INSERT INTO federation_peer
         (node_id, peer_id, public_key, cluster, region, trust_state, software_version, epoch)
         VALUES ('ingest-node', 'ingest-node', repeat('a', 64), 'cluster', 'region',
                 'authorized', '1.0.0', 1);
         INSERT INTO federation_peer_scope (node_id, tenant_id, stream_id)
         VALUES ('ingest-node', 'tenant', 'stream');",
    )
    .execute(&pool)
    .await
    .unwrap();
    let scope = "SELECT to_jsonb(s)::text FROM federation_peer_scope s";
    let prior_scope: String = sqlx::query_scalar(scope).fetch_one(&pool).await.unwrap();
    let before = control_rows(&pool).await;
    assert!(
        before
            .iter()
            .any(|row| row.0 == "0010_event_signed_ingest.sql")
    );
    let report = migrate::migrate(&pool, &migrations_dir()).await.unwrap();
    assert_eq!(
        report.applied,
        [
            "0009_federation_approval.sql",
            "0011_federation_decision_epoch.sql"
        ]
    );
    let after = control_rows(&pool).await;
    assert_eq!(after.len(), before.len() + 2);
    assert!(before.iter().all(|row| after.contains(row)));
    assert!(run_migrate_only(&url, &migrations_dir()).status.success());
    assert_eq!(control_rows(&pool).await, after);
    let current_scope: String = sqlx::query_scalar(scope).fetch_one(&pool).await.unwrap();
    assert_eq!(current_scope, prior_scope);
}

#[tokio::test]
async fn m1bi_legacy_approval_blocks_0009_without_partial_changes() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let (url, pool) = schema_without(&base, &["0009", "0011"]).await;
    sqlx::query(
        "INSERT INTO federation_join_request
         (join_request_id, node_id, epoch, manifest, state, approved_at, approved_by)
         VALUES ('fjr_00000000000000000000000000000001', 'legacy', 1, '{}',
                 'approved', now(), 'operator')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let before = control_rows(&pool).await;
    let request = "SELECT to_jsonb(r)::text FROM federation_join_request r";
    let legacy: String = sqlx::query_scalar(request).fetch_one(&pool).await.unwrap();

    let result = run_migrate_only(&url, &migrations_dir());
    assert!(
        !result.status.success(),
        "0009 precisa bloquear aprovação legada"
    );
    let error = stderr_of(&result);
    assert!(
        error.contains("0009_federation_approval.sql") && error.contains("sem prova assinada"),
        "{error}"
    );
    assert_eq!(control_rows(&pool).await, before);
    let after: String = sqlx::query_scalar(request).fetch_one(&pool).await.unwrap();
    assert_eq!(after, legacy);
    let index: Option<String> = sqlx::query_scalar(
        "SELECT indexname FROM pg_indexes WHERE tablename = 'federation_join_request'
         AND indexdef LIKE '%approval_epoch%'",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(index, None);
}

#[test]
fn m1bi_governance_malformada_impede_boot_do_servico() {
    for value in [
        "quorum:cluster",
        "single_approver:",
        "single_approver:a:b",
        " ",
    ] {
        let result = Command::new(binario())
            .env("DATABASE_URL", "not-a-postgres-url")
            .env("GODMESH_GRANTS", "{\"grants\":[]}")
            .env("BIND_ADDR", "127.0.0.1:0")
            .env("GODMESH_FEDERATION_GOVERNANCE", value)
            .env_remove("GODMESH_FEDERATION_GENESIS")
            .env_remove("GODMESH_FEDERATION_BIND")
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(78), "{}", stderr_of(&result));
        let error = stderr_of(&result);
        assert!(
            error.contains("GODMESH_FEDERATION_GOVERNANCE"),
            "{value:?}: {error}"
        );
    }
}

#[tokio::test]
async fn m1bi_0011_inicializa_piso_por_aprovador_e_preserva_historico() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    let (_, pool) = schema_without(&base, &["0011"]).await;
    sqlx::raw_sql(
        "INSERT INTO federation_join_request
            (join_request_id, node_id, epoch, manifest, state, approved_at, approved_by,
             approved_by_node, approval_signature, approval_epoch, approval_expires_at)
         SELECT 'fjr_' || lpad(n::text, 32, '0'), 'candidate-' || n, 1, '{}', 'approved',
                now(), 'operator', 'gmn_' || repeat(signer, 64), repeat('a', 128), epoch,
                now() + interval '1 day'
           FROM (VALUES (1, 'a', 9), (2, 'a', 3), (3, 'b', 7)) AS history(n, signer, epoch);
         INSERT INTO federation_join_request (join_request_id, node_id, epoch, manifest, state)
         VALUES ('fjr_00000000000000000000000000000004', 'candidate-4', 1, '{}', 'rejected');",
    )
    .execute(&pool)
    .await
    .unwrap();

    let history =
        "SELECT to_jsonb(r)::text FROM federation_join_request r ORDER BY join_request_id";
    let requests: Vec<String> = sqlx::query_scalar(history).fetch_all(&pool).await.unwrap();
    let before = control_rows(&pool).await;
    let report = migrate::migrate(&pool, &migrations_dir()).await.unwrap();
    assert_eq!(report.applied, ["0011_federation_decision_epoch.sql"]);
    let after = control_rows(&pool).await;
    assert_eq!(after.len(), before.len() + 1);
    assert!(before.iter().all(|row| after.contains(row)));
    let floor = "SELECT approver_node_id, decision_epoch FROM federation_decision_epoch ORDER BY 1";
    let expected = [
        (format!("gmn_{}", "a".repeat(64)), 9),
        (format!("gmn_{}", "b".repeat(64)), 7),
    ];
    let floors: Vec<(String, i64)> = sqlx::query_as(floor).fetch_all(&pool).await.unwrap();
    assert_eq!(floors, expected);
    let same_requests: Vec<String> = sqlx::query_scalar(history).fetch_all(&pool).await.unwrap();
    assert_eq!(same_requests, requests);

    let rerun = migrate::migrate(&pool, &migrations_dir()).await.unwrap();
    assert!(rerun.applied.is_empty());
    assert_eq!(control_rows(&pool).await, after);
    let same_floors: Vec<(String, i64)> = sqlx::query_as(floor).fetch_all(&pool).await.unwrap();
    assert_eq!(same_floors, floors);
}

#[tokio::test]
async fn m4a1_aplica_0014_sem_converter_posicao_e_checkpoint_legados() {
    let Some(base) = base_url() else {
        eprintln!("SKIP: DATABASE_URL ausente");
        return;
    };
    let _guard = sole_lock().await;
    recreate_sa(&admin_pool(&base).await).await;
    let pool = pool_for(&sa_url(&base)).await;
    let dir = migrations_dir();
    const REPLICA_MIGRATION: &str = "0014_federation_replication.sql";
    let prior = std::env::temp_dir().join(format!("godmesh-m4a1-{}", std::process::id()));
    std::fs::create_dir(&prior).unwrap();
    for name in nomes_de_migracao(&dir) {
        if name.as_str() < REPLICA_MIGRATION {
            std::fs::copy(dir.join(&name), prior.join(&name)).unwrap();
        }
    }
    migrate::migrate(&pool, &prior).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO event_position
            (origin_node, tenant_id, stream_id, partition, authority_epoch, seq,
             retention_class, jurisdiction)
         VALUES ('legacy-node', 'tenant', 'm4a.metadata.legado', 0, 7, 1, 'mensagem', 'BR');
         INSERT INTO sync_inbox_state
            (origin_node, tenant_id, stream_id, partition, authority_epoch, contiguous_prefix)
         VALUES ('legacy-node', 'tenant', 'm4a.metadata.legado', 0, 7, 1);",
    )
    .execute(&pool)
    .await
    .unwrap();
    let snapshot = "SELECT jsonb_build_array(
        (SELECT to_jsonb(e) FROM event_position e WHERE origin_node = 'legacy-node'),
        (SELECT to_jsonb(s) FROM sync_inbox_state s WHERE origin_node = 'legacy-node')
    )::text";
    let rows: String = sqlx::query_scalar(snapshot).fetch_one(&pool).await.unwrap();
    let before = control_rows(&pool).await;
    assert_eq!(before.len(), 13);

    std::fs::copy(dir.join(REPLICA_MIGRATION), prior.join(REPLICA_MIGRATION)).unwrap();
    let report = migrate::migrate(&pool, &prior).await.unwrap();
    assert_eq!(report.applied, [REPLICA_MIGRATION]);
    let after = control_rows(&pool).await;
    assert_eq!(&after[..13], before.as_slice());
    let rerun = migrate::migrate(&pool, &prior).await.unwrap();
    assert!(rerun.applied.is_empty());
    assert_eq!(control_rows(&pool).await, after);
    let current: String = sqlx::query_scalar(snapshot).fetch_one(&pool).await.unwrap();
    assert_eq!(current, rows);
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM federation_replica_namespace),
                (SELECT count(*) FROM federation_replica_proof)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (0, 0));
    std::fs::remove_dir_all(prior).unwrap();
}
