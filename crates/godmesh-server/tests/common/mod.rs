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
