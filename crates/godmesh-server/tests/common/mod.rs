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

// ---------------------------------------------------------------------------
// S-sec-2 — envelopes de evento assinados pelo nó.
//
// Todo `POST /v1/events` de teste passa por aqui: não existe mais corpo sem
// assinatura na rota, e os testes provam isso por construção (a única forma de
// fabricar um corpo válido é assinar). A criptografia é a do próprio
// `godmesh-federation` — o teste não implementa nada.
// ---------------------------------------------------------------------------

use godmesh_federation::event::EventDraft;
use godmesh_federation::{NodeIdentity, now_unix, signed_event};

/// Nó **local** do processo de teste: a chave vive só aqui, e o `node_id` é o
/// derivado da chave pública — a mesma derivação que a produção lê de
/// `GODMESH_NODE_KEY`.
pub static LOCAL_NODE: LazyLock<NodeIdentity> = LazyLock::new(|| {
    NodeIdentity::generate_in_memory().expect("identidade de nó em memória")
});

// Upserts simultâneos da fixture podem colidir no UNIQUE de peer_id antes
// de o ON CONFLICT (node_id) resolver a disputa.
static LOCAL_NODE_REGISTRATION: Mutex<()> = Mutex::const_new(());

/// Registro o nó em `federation_peer` com o estado pedido (idempotente por
/// `node_id`, como o ingresso). `public_key` é a chave do arquivo do nó —
/// dado público; a privada nunca sai do processo.
pub async fn registrar_no(pool: &PgPool, identity: &NodeIdentity, trust_state: &str) {
    sqlx::query(
        "INSERT INTO federation_peer \
             (node_id, peer_id, public_key, cluster, region, trust_state, \
              software_version, epoch) \
         VALUES ($1, $1, $2, 'cluster-teste', 'BR', $3, '0.0.1', 1) \
         ON CONFLICT (node_id) DO UPDATE SET trust_state = EXCLUDED.trust_state",
    )
    .bind(identity.node_id())
    .bind(identity.public_key_hex())
    .bind(trust_state)
    .execute(pool)
    .await
    .expect("par de teste registrado");
}

/// Concede o escopo exato `(nó, tenant, stream)` — sem linha, o ingest nega.
pub async fn conceder_escopo(pool: &PgPool, node_id: &str, tenant_id: &str, stream_id: &str) {
    sqlx::query(
        "INSERT INTO federation_peer_scope (node_id, tenant_id, stream_id) \
         VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(node_id)
    .bind(tenant_id)
    .bind(stream_id)
    .execute(pool)
    .await
    .expect("escopo de ingest concedido");
}

/// Nó local autorizado + escopo para `(tenant, stream)`: a preparação mínima
/// de um caminho feliz. Idempotente para testes que chamam mais de uma vez.
pub async fn no_local_pronto(pool: &PgPool, tenant_id: &str, stream_id: &str) {
    {
        let _registration = LOCAL_NODE_REGISTRATION.lock().await;
        registrar_no(pool, &LOCAL_NODE, "authorized").await;
    }
    conceder_escopo(pool, LOCAL_NODE.node_id(), tenant_id, stream_id).await;
}

/// Envelope assinado pelo nó, JSON pronto para o corpo de `POST /v1/events`.
pub fn envelope_assinado(
    identity: &NodeIdentity,
    tenant_id: &str,
    stream_id: &str,
    partition: u32,
    authority_epoch: u64,
    seq: u64,
    retention_class: &str,
) -> String {
    let event = signed_event(
        identity,
        EventDraft {
            tenant_id,
            stream_id,
            partition,
            authority_epoch,
            seq,
            retention_class,
            issued_at: now_unix(),
        },
    );
    serde_json::to_string(&event).expect("envelope serializável")
}

/// Envelope com `issued_at` explícito (cenários de janela de frescor): o
/// chamador constrói o [`EventDraft`] e recebe o JSON já assinado. Alterar o
/// corpo depois de assinar quebraria a assinatura — o teste provaria outra
/// coisa, então a API só permite assinar o rascunho inteiro de uma vez.
pub fn envelope(identity: &NodeIdentity, draft: EventDraft<'_>) -> String {
    serde_json::to_string(&signed_event(identity, draft)).expect("envelope serializável")
}
