//! ADR-061 — contrato do outbox **persistente**: publicar não é confirmar.
//!
//! Cada teste usa um grupo `(origin, tenant, stream, partition, epoch)` próprio,
//! então os testes deste binário podem rodar em paralelo no mesmo banco.
//!
//! Banco: `GODMESH_TEST_DATABASE_URL` (ou `DATABASE_URL`). O esquema é aplicado
//! do zero — migrações 0001, 0002 e 0003 — quando `outbox_sink` ainda não existe.
//! Sem URL, cada teste é pulado com `SKIP:`; nenhum teste finge sucesso.
//!
//!   psql "$ADMIN" -c 'CREATE DATABASE godmesh_sd'
//!   GODMESH_TEST_DATABASE_URL=postgres://godmesh:godmesh_dev_only@localhost:55432/godmesh_sd \
//!     cargo test -p godmesh-server --test outbox_publicador -- --test-threads=4

use std::sync::atomic::{AtomicU64, Ordering};

use godmesh_server::publisher::{Publisher, Stats};
use godmesh_sync::pg_outbox::{
    OutboxError, PartitionKey, PendingEntry, PersistedEffect, PgOutbox, PgSink, PublishOutcome, Sink,
    SinkError,
};
use sqlx::PgPool;

/// Sequência só para gerar identidades únicas por chamada.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Schema esperado: a presença de `outbox_sink` indica a migração 0003 aplicada.
const MIGRATIONS: &[&str] = &[
    include_str!("../../../migrations/0001_core.sql"),
    include_str!("../../../migrations/0002_incident_tenancy.sql"),
    include_str!("../../../migrations/0003_outbox_publisher.sql"),
];

async fn pool() -> Option<PgPool> {
    let url = std::env::var("GODMESH_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .ok()?;
    ensure_schema(&pool).await;
    Some(pool)
}

/// Aplica as migrações quando o esquema ainda não existe, sob lock consultivo
/// para que dois binários de teste não apliquem 0001 ao mesmo tempo.
async fn ensure_schema(pool: &PgPool) {
    let existe: bool = sqlx::query_scalar("SELECT to_regclass('public.outbox_sink') IS NOT NULL")
        .fetch_one(pool)
        .await
        .expect("consulta de esquema");
    if existe {
        return;
    }

    sqlx::query("SELECT pg_advisory_lock(hashtextextended('godmesh.test.schema', 0))")
        .execute(pool)
        .await
        .expect("lock de esquema");

    let ainda_nao: bool = sqlx::query_scalar("SELECT to_regclass('public.outbox_sink') IS NULL")
        .fetch_one(pool)
        .await
        .expect("recheque de esquema");
    if ainda_nao {
        for migracao in MIGRATIONS {
            sqlx::raw_sql(*migracao)
                .execute(pool)
                .await
                .expect("migração aplicada");
        }
    }

    sqlx::query("SELECT pg_advisory_unlock(hashtextextended('godmesh.test.schema', 0))")
        .execute(pool)
        .await
        .expect("unlock de esquema");
}

/// Grupo isolado por teste.
fn chave(test: &str) -> PartitionKey {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let origin = format!("node-{test}-{}-{n}", std::process::id());
    PartitionKey::new(&origin, "tenant-sd", "stream-outbox", 1, 7).expect("chave válida")
}

/// SQL literal por tabela: `sqlx` 0.9 exige string estática (`SqlSafeStr`).
async fn limpar(pool: &PgPool, key: &PartitionKey) {
    for sql in [
        "DELETE FROM outbox_entry WHERE origin_node = $1 AND tenant_id = $2 \
         AND stream_id = $3 AND partition = $4 AND authority_epoch = $5",
        "DELETE FROM outbox_sink WHERE origin_node = $1 AND tenant_id = $2 \
         AND stream_id = $3 AND partition = $4 AND authority_epoch = $5",
    ] {
        sqlx::query(sql)
            .bind(key.origin_node())
            .bind(key.tenant_id())
            .bind(key.stream_id())
            .bind(key.partition())
            .bind(key.authority_epoch())
            .execute(pool)
            .await
            .expect("limpeza do grupo de teste");
    }
}

async fn tentativas(pool: &PgPool, key: &PartitionKey, seq: i64) -> (i32, bool, bool) {
    sqlx::query_as::<_, (i32, bool, bool)>(
        "SELECT attempts, published_at IS NOT NULL, confirmed_at IS NOT NULL \
           FROM outbox_entry \
          WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
            AND partition = $4 AND authority_epoch = $5 AND seq = $6",
    )
    .bind(key.origin_node())
    .bind(key.tenant_id())
    .bind(key.stream_id())
    .bind(key.partition())
    .bind(key.authority_epoch())
    .bind(seq)
    .fetch_one(pool)
    .await
    .expect("linha do outbox")
}

async fn total_sink(pool: &PgPool, key: &PartitionKey) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM outbox_sink \
          WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
            AND partition = $4 AND authority_epoch = $5",
    )
    .bind(key.origin_node())
    .bind(key.tenant_id())
    .bind(key.stream_id())
    .bind(key.partition())
    .bind(key.authority_epoch())
    .fetch_one(pool)
    .await
    .expect("contagem do sink")
}

/// Sink que sempre recusa: prova que falha de efeito não confirma nada.
#[derive(Debug)]
struct SinkQueFalha;

impl Sink for SinkQueFalha {
    async fn deliver(&self, _key: &PartitionKey, entry: &PendingEntry) -> Result<(), SinkError> {
        Err(SinkError(format!("sink de teste recusou a seq {}", entry.seq)))
    }
}

#[tokio::test]
async fn append_publish_confirm_em_ordem() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: GODMESH_TEST_DATABASE_URL/DATABASE_URL ausente");
        return;
    };
    let key = chave("ordem");
    limpar(&pool, &key).await;
    let outbox = PgOutbox::new(pool.clone());

    for seq in 1..=3 {
        outbox
            .append(&key, seq, PersistedEffect::Event)
            .await
            .expect("append contíguo");
    }

    let scan = outbox.pending(&key).await.expect("pendências");
    assert_eq!(scan.confirmed_prefix, 0, "nada confirmado ainda");
    assert_eq!(scan.entries.len(), 3, "três pendências contíguas");
    assert_eq!(scan.missing, None);

    let primeira = outbox.publish_next(&key).await.expect("publicação");
    let PublishOutcome::Published(entrada) = primeira else {
        panic!("a seq 1 precisa ser publicada: {primeira:?}");
    };
    assert_eq!(entrada.seq, 1);
    assert!(!entrada.is_irreversible());
    assert_eq!(
        outbox.confirmed_prefix(&key).await.expect("prefixo"),
        0,
        "publicar não é confirmar"
    );
    assert_eq!(
        outbox.confirm(&key, 1).await.map_err(|e| e.to_string()),
        Ok(()),
        "confirmar depois do efeito"
    );
    assert_eq!(outbox.confirmed_prefix(&key).await.expect("prefixo"), 1);

    let scan = outbox.pending(&key).await.expect("pendências");
    assert_eq!(scan.entries.len(), 2, "restam 2 e 3");
    assert_eq!(scan.entries.first().map(|e| e.seq), Some(2));

    let segunda = outbox.publish_next(&key).await.expect("publicação");
    assert!(
        matches!(segunda, PublishOutcome::Published(ref e) if e.seq == 2),
        "publicação segue a ordem da sequência: {segunda:?}"
    );
    outbox.confirm(&key, 2).await.expect("confirmação da seq 2");
    assert_eq!(outbox.confirmed_prefix(&key).await.expect("prefixo"), 2);

    limpar(&pool, &key).await;
}

#[tokio::test]
async fn confirm_sem_publish_falha() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: GODMESH_TEST_DATABASE_URL/DATABASE_URL ausente");
        return;
    };
    let key = chave("confirm-sem-publish");
    limpar(&pool, &key).await;
    let outbox = PgOutbox::new(pool.clone());

    outbox
        .append(&key, 1, PersistedEffect::Event)
        .await
        .expect("append");

    let erro = outbox.confirm(&key, 1).await.expect_err("confirmar sem publicar");
    assert!(
        matches!(erro, OutboxError::EntryNotPublished { seq: 1 }),
        "a recusa precisa ser explícita: {erro}"
    );
    assert_eq!(
        outbox.confirmed_prefix(&key).await.expect("prefixo"),
        0,
        "recusa não confirma nada"
    );
    let (attempts, published, confirmed) = tentativas(&pool, &key, 1).await;
    assert_eq!((attempts, published, confirmed), (0, false, false));

    // A mesma recusa vale para sequência inexistente.
    let erro = outbox.confirm(&key, 9).await.expect_err("confirmar o que não existe");
    assert!(matches!(erro, OutboxError::EntryNotFound { seq: 9 }), "{erro}");

    limpar(&pool, &key).await;
}

#[tokio::test]
async fn lacuna_interrompe_publicacao() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: GODMESH_TEST_DATABASE_URL/DATABASE_URL ausente");
        return;
    };
    let key = chave("lacuna");
    limpar(&pool, &key).await;
    let outbox = PgOutbox::new(pool.clone());

    // `append` recusa lacuna, então a lacuna é injetada direto na tabela — é o
    // estado inconsistente que o publicador precisa recusar, não produzir.
    for seq in [1_i64, 3] {
        sqlx::query(
            "INSERT INTO outbox_entry \
                 (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, effect_kind) \
             VALUES ($1, $2, $3, $4, $5, $6, 'event')",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(seq)
        .execute(&pool)
        .await
        .expect("linha com lacuna");
    }

    let scan = outbox.pending(&key).await.expect("pendências");
    assert_eq!(
        scan.entries.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![1],
        "a leitura para na lacuna"
    );
    assert_eq!(scan.missing, Some(2), "a sequência ausente é nomeada");

    let primeira = outbox.publish_next(&key).await.expect("publicação");
    assert!(matches!(primeira, PublishOutcome::Published(ref e) if e.seq == 1), "{primeira:?}");
    outbox.confirm(&key, 1).await.expect("confirmação da seq 1");

    let bloqueada = outbox.publish_next(&key).await.expect("publicação bloqueada");
    assert_eq!(
        bloqueada,
        PublishOutcome::BlockedByGap { missing: 2 },
        "nada depois da lacuna é publicado"
    );
    let (_, publicado_3, confirmado_3) = tentativas(&pool, &key, 3).await;
    assert!(
        !publicado_3 && !confirmado_3,
        "a seq 3 continua intocada atrás da lacuna"
    );

    // Confirmar a sequência depois da lacuna também é recusado.
    let erro = outbox
        .confirm(&key, 3)
        .await
        .expect_err("confirmar atrás da lacuna");
    assert!(
        matches!(erro, OutboxError::EntryNotPublished { seq: 3 }),
        "{erro}"
    );

    limpar(&pool, &key).await;
}

#[tokio::test]
async fn efeito_irreversivel_nao_e_reenviado() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: GODMESH_TEST_DATABASE_URL/DATABASE_URL ausente");
        return;
    };
    let key = chave("irreversivel");
    limpar(&pool, &key).await;
    let outbox = PgOutbox::new(pool.clone());

    outbox
        .append(&key, 1, PersistedEffect::Irreversible)
        .await
        .expect("append irreversível");

    let primeira = outbox.publish_next(&key).await.expect("publicação");
    assert!(
        matches!(primeira, PublishOutcome::Published(ref e) if e.seq == 1 && e.is_irreversible()),
        "{primeira:?}"
    );

    // O efeito falhou depois de reivindicado: resultado externo desconhecido.
    let falha = outbox.record_failure(&key, 1).await.expect("registro de falha");
    assert_eq!(falha.attempts, 1, "a tentativa é contada");
    assert!(
        !falha.retry_scheduled,
        "efeito irreversível não volta para a fila"
    );

    let (attempts, published, confirmed) = tentativas(&pool, &key, 1).await;
    assert_eq!(attempts, 1);
    assert!(published, "a reivindicação permanece registrada");
    assert!(!confirmed, "falha do efeito não confirma");

    // Qualquer nova passagem vê reconciliação pendente, nunca reentrega.
    for _ in 0..3 {
        let desfecho = outbox.publish_next(&key).await.expect("nova publicação");
        assert!(
            matches!(desfecho, PublishOutcome::AwaitingReconciliation(ref e) if e.seq == 1),
            "efeito irreversível não é reenviado: {desfecho:?}"
        );
    }
    assert_eq!(total_sink(&pool, &key).await, 0, "nada foi entregue");

    limpar(&pool, &key).await;
}

#[tokio::test]
async fn duas_instancias_nao_duplicam_publicacao() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: GODMESH_TEST_DATABASE_URL/DATABASE_URL ausente");
        return;
    };
    let key = chave("duas-instancias");
    limpar(&pool, &key).await;
    let total = 12_i64;
    for seq in 1..=total {
        PgOutbox::new(pool.clone())
            .append(&key, seq, PersistedEffect::Event)
            .await
            .expect("append");
    }

    // Duas instâncias concorrentes, cada uma com o próprio sink e o próprio
    // `PgOutbox`, publicando e confirmando até não sobrar nada contíguo.
    let mut tarefas = Vec::new();
    for instancia in 0..2 {
        let pool = pool.clone();
        let key = key.clone();
        tarefas.push(tokio::spawn(async move {
            let outbox = PgOutbox::new(pool.clone());
            let sink = PgSink::new(pool.clone());
            let mut reivindicadas = Vec::new();
            // Para em qualquer desfecho que não seja publicação: sem trabalho
            // contíguo, em disputa com a outra instância ou bloqueada.
            while let PublishOutcome::Published(entrada) =
                outbox.publish_next(&key).await.expect("publicação")
            {
                reivindicadas.push(entrada.seq);
                // Sink com PK por posição: entrega duplicada falharia aqui.
                sink.deliver(&key, &entrada)
                    .await
                    .unwrap_or_else(|e| panic!("instância {instancia}: {e}"));
                outbox
                    .confirm(&key, entrada.seq)
                    .await
                    .unwrap_or_else(|e| panic!("instância {instancia}: {e}"));
            }
            reivindicadas
        }));
    }

    let mut reivindicadas = Vec::new();
    for tarefa in tarefas {
        reivindicadas.extend(tarefa.await.expect("tarefa do publicador"));
    }

    // Drenagem final: garante que o teste mede cobertura, não velocidade.
    let outbox = PgOutbox::new(pool.clone());
    let sink = PgSink::new(pool.clone());
    while let PublishOutcome::Published(entrada) =
        outbox.publish_next(&key).await.expect("drenagem")
    {
        reivindicadas.push(entrada.seq);
        sink.deliver(&key, &entrada).await.expect("entrega na drenagem");
        outbox.confirm(&key, entrada.seq).await.expect("confirmação na drenagem");
    }

    let mut ordenadas = reivindicadas.clone();
    ordenadas.sort_unstable();
    let mut sem_repeticao = ordenadas.clone();
    sem_repeticao.dedup();
    assert_eq!(
        ordenadas, sem_repeticao,
        "nenhuma sequência pode ser publicada duas vezes: {reivindicadas:?}"
    );
    assert_eq!(
        ordenadas,
        (1..=total).collect::<Vec<_>>(),
        "todas as sequências foram publicadas uma única vez"
    );
    assert_eq!(total_sink(&pool, &key).await, total, "uma entrega por posição");
    assert_eq!(
        outbox.confirmed_prefix(&key).await.expect("prefixo"),
        total,
        "prefixo contíguo fechado"
    );

    limpar(&pool, &key).await;
}

#[tokio::test]
async fn falha_do_sink_mantem_pendente_com_attempts() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: GODMESH_TEST_DATABASE_URL/DATABASE_URL ausente");
        return;
    };
    let key = chave("falha-sink");
    limpar(&pool, &key).await;
    let outbox = PgOutbox::new(pool.clone());
    outbox
        .append(&key, 1, PersistedEffect::Event)
        .await
        .expect("append");
    outbox
        .append(&key, 2, PersistedEffect::Event)
        .await
        .expect("append");

    let publisher = Publisher::new(PgOutbox::new(pool.clone()), SinkQueFalha);
    let stats: Stats = publisher.run_once_for(&key).await.expect("passagem do publicador");
    assert!(
        stats.failed >= 1,
        "a falha do sink precisa ser contabilizada: {stats:?}"
    );

    let (attempts, published, confirmed) = tentativas(&pool, &key, 1).await;
    assert_eq!(attempts, 1, "a tentativa é incrementada");
    assert!(!confirmed, "a linha continua sem confirmação");
    assert!(
        !published,
        "evento volta para a fila: published_at é limpo para nova tentativa"
    );

    // Segunda passagem: a tentativa acumula, a linha continua pendente.
    publisher.run_once_for(&key).await.expect("segunda passagem");
    let (attempts, _, confirmed) = tentativas(&pool, &key, 1).await;
    assert_eq!(attempts, 2, "tentativas acumulam");
    assert!(!confirmed);

    // Com um sink que aceita, a mesma linha é publicada e confirmada.
    let publisher = Publisher::new(PgOutbox::new(pool.clone()), PgSink::new(pool.clone()));
    let stats = publisher.run_once_for(&key).await.expect("passagem com sink bom");
    assert_eq!(stats.confirmed, 2, "as duas entradas deste grupo são confirmadas");
    assert_eq!(outbox.confirmed_prefix(&key).await.expect("prefixo"), 2);
    let (attempts, _, confirmed) = tentativas(&pool, &key, 1).await;
    assert_eq!(attempts, 2, "a contagem de falhas não é apagada pelo sucesso");
    assert!(confirmed);

    limpar(&pool, &key).await;
}
#[tokio::test]
async fn loop_publica_e_encerra_no_sinal() {
    let Some(pool) = pool().await else {
        eprintln!("SKIP: GODMESH_TEST_DATABASE_URL/DATABASE_URL ausente");
        return;
    };
    let key = chave("loop");
    limpar(&pool, &key).await;
    let outbox = PgOutbox::new(pool.clone());
    for seq in 1..=2 {
        outbox
            .append(&key, seq, PersistedEffect::Event)
            .await
            .expect("append");
    }

    let publisher = Publisher::new(PgOutbox::new(pool.clone()), PgSink::new(pool.clone()));
    // Intervalo curto de propósito: o teste mede a ligação do laço com o sinal de
    // término, não o valor de produção (default 1000 ms, coberto em unit test).
    let total = publisher
        .run_for(&key, std::time::Duration::from_millis(20), async {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        })
        .await;

    assert!(total.confirmed >= 2, "o laço confirmou o que publicou: {total:?}");
    assert_eq!(
        outbox.confirmed_prefix(&key).await.expect("prefixo"),
        2,
        "o laço fechou o prefixo contíguo"
    );
    assert_eq!(total_sink(&pool, &key).await, 2, "uma entrega por posição");

    limpar(&pool, &key).await;
}
