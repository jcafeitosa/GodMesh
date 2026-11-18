//! Suporte comum dos testes de integração que dependem de esquema.
//!
//! Regra: **nenhum teste assume esquema pronto**. O banco é migrado pelo
//! *runner oficial* (`godmesh_server::migrate::migrate`, o mesmo que o binário
//! usa em `--migrate-only` e na inicialização), sempre a partir do diretório
//! `migrations/` do repositório. Assim o teste exercita a mesma ordem, o mesmo
//! controle (`schema_migration`) e o mesmo checksum que a produção.
//!
//! Isso substitui o antigo "aplique 0001..000N na mão antes de rodar": um teste
//! que falhava por tabela ausente acusava o setup, não o código.
//!
//! Sem `GODMESH_TEST_DATABASE_URL`/`DATABASE_URL` o teste é pulado (devolve
//! `None`). Com URL declarada e banco inacessível, o teste **falha**: banco
//! configurado e inalcançável não pode parecer verde.

#![allow(dead_code)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use sqlx::PgPool;
use tokio::sync::Mutex;

/// URL do banco de teste: `GODMESH_TEST_DATABASE_URL` tem precedência.
pub fn database_url() -> Option<String> {
    std::env::var("GODMESH_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
}

/// Diretório de migrações do repositório, independente do `cwd` do `cargo test`.
pub fn migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("migrations")
}

/// URLs já migradas nesta execução do binário de teste.
static MIGRADOS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Pool migrado pelo runner oficial, ou `None` sem URL configurada.
pub async fn pool() -> Option<PgPool> {
    pool_com_conexoes(4).await
}

/// Igual a [`pool`], com um teto de conexões escolhido pelo teste.
pub async fn pool_com_conexoes(max_connections: u32) -> Option<PgPool> {
    let url = database_url()?;
    Some(pool_para(&url, max_connections).await)
}

/// Pool para uma URL explícita, já migrado. Usado por testes que criam o próprio
/// banco e precisam do esquema garantido nele.
///
/// # Panics
/// Conexão impossível ou falha do runner de migrações. Os dois são erro de
/// ambiente declarado, nunca "skip" silencioso.
pub async fn pool_para(url: &str, max_connections: u32) -> PgPool {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .unwrap_or_else(|e| panic!("banco de teste inacessível ({url}): {e}"));
    migrar(&pool, url).await;
    pool
}

/// Aplica as migrações pendentes pelo runner oficial, uma vez por URL.
async fn migrar(pool: &PgPool, url: &str) {
    let mut migrados = MIGRADOS.lock().await;
    if migrados.contains(url) {
        return;
    }
    let report = godmesh_server::migrate::migrate(pool, &migrations_dir())
        .await
        .unwrap_or_else(|e| panic!("runner oficial de migrações falhou ({url}): {e}"));
    eprintln!(
        "migrações ({url}): aplicadas {:?}, já presentes {:?}",
        report.applied,
        report.skipped_count()
    );
    migrados.insert(url.to_owned());
}

// --- cliente HTTP/1.1 de teste, sem dependência nova -------------------------

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Resposta HTTP como o teste precisa vê-la: status e corpo cru.
pub struct HttpResponse {
    /// Código de status.
    pub status: u16,
    /// Corpo integral.
    pub body: String,
}

impl HttpResponse {
    /// Corpo como JSON; falha alto se não for.
    ///
    /// # Panics
    /// Corpo não-JSON, com o status junto — nunca um "skip" silencioso.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or_else(|e| {
            panic!(
                "corpo não é JSON ({e}): {:?} (status {})",
                self.body, self.status
            )
        })
    }
}

/// Executa um pedido HTTP/1.1 completo. `token = None` faz pedido **sem**
/// `Authorization`, que é como se prova rota pública.
///
/// # Panics
/// Falha de rede ou resposta HTTP malformada.
pub fn call(
    addr: SocketAddr,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> HttpResponse {
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
    let status: u16 = head
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {raw:?}"));
    HttpResponse {
        status,
        body: body.to_owned(),
    }
}

/// GET autenticado.
///
/// # Panics
/// Falha de rede.
pub fn get(addr: SocketAddr, path: &str, token: &str) -> HttpResponse {
    call(addr, "GET", path, Some(token), None)
}
