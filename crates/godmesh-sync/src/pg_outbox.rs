//! ADR-061 — outbox **persistente** em Postgres: publicar não é confirmar.
//!
//! O que este módulo garante, e como:
//!
//! - **append transacional**: a entrada nasce na tabela `outbox_entry` sem
//!   `published_at` e sem `confirmed_at`; a contiguidade da sequência dentro do
//!   grupo `(origin, tenant, stream, partition, epoch)` é verificada sob
//!   `pg_advisory_xact_lock`, então duas escritas concorrentes no mesmo grupo não
//!   produzem lacuna nem sequência repetida;
//! - **publicar ≠ confirmar**: `publish_next` só grava `published_at`, com
//!   `UPDATE … RETURNING` sobre a linha reivindicada sob `FOR UPDATE SKIP
//!   LOCKED`. A reivindicação é atômica: duas instâncias do publicador nunca
//!   levam a mesma linha;
//! - **confirmação só em ordem e só depois do efeito**: `confirm` recusa
//!   sequência sem `published_at` (`EntryNotPublished`), sequência inexistente
//!   (`EntryNotFound`) e sequência fora do prefixo contíguo
//!   (`ConfirmationOutOfOrder`). O banco repete a regra na constraint
//!   `outbox_entry_confirmed_requires_published` (migração 0003);
//! - **lacuna não é publicada**: o publicador só considera `seq = prefixo + 1`,
//!   onde o prefixo é o maior `seq` confirmado do grupo. `seq` ausente no meio
//!   interrompe tudo o que vem depois (`PublishOutcome::BlockedByGap`);
//! - **efeito irreversível não é reenviado**: `publish_next` só reivindica
//!   `published_at IS NULL`. Uma falha de efeito irrigável devolve a linha para a
//!   fila (`published_at = NULL`); uma falha de efeito **irreversível** mantém
//!   `published_at` marcado e a linha aparece como
//!   `PublishOutcome::AwaitingReconciliation`, nunca é reentregue;
//! - **falha do efeito**: `record_failure` incrementa `attempts` e deixa
//!   `confirmed_at` nulo.
//!
//! Limite declarado: `outbox_entry` guarda sequência e natureza do efeito, não
//! payload. O efeito desta fase é o sink de teste `outbox_sink` (migração 0003).
//! Não existe broker, bridge, rádio, pagamento ou notificação aqui — e nenhum
//! teste deste módulo prova entrega a eles.

use core::fmt;
use core::future::Future;

use sqlx::PgPool;
use sqlx::Row;

/// Natureza do efeito persistido em `outbox_entry.effect_kind`.
///
/// Espelha o `CHECK` da migração 0001 (`event`,`irreversible`). É um tipo de
/// armazenamento: o rótulo livre de [`godmesh_sync::outbox::EffectKind`] (por
/// exemplo `bridge_tx`) não é persistido nesta fatia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedEffect {
    /// Evento de domínio. Retentativa é permitida: reentrega é at-least-once.
    Event,
    /// Efeito externo irreversível. Depois de reivindicado, não é reenviado.
    Irreversible,
}

impl PersistedEffect {
    /// Nome gravado na coluna `effect_kind`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Irreversible => "irreversible",
        }
    }

    /// Converte o nome gravado; `None` para valor fora do `CHECK`.
    #[must_use]
    pub const fn parse(name: &str) -> Option<Self> {
        match name.as_bytes() {
            b"event" => Some(Self::Event),
            b"irreversible" => Some(Self::Irreversible),
            _ => None,
        }
    }
}

/// Identidade de uma partição de outbox (ADR-057/ADR-061).
///
/// A sequência é contígua **dentro** deste grupo. `authority_epoch` faz parte do
/// grupo de propósito: um epoch novo abre um espaço de sequência novo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionKey {
    origin_node: String,
    tenant_id: String,
    stream_id: String,
    partition: i32,
    authority_epoch: i64,
}

impl PartitionKey {
    /// # Errors
    /// `InvalidKey` para identificadores vazios, `partition` acima de `i32::MAX`
    /// ou `authority_epoch` fora de `1..=i64::MAX`.
    pub fn new(
        origin_node: &str,
        tenant_id: &str,
        stream_id: &str,
        partition: u32,
        authority_epoch: u64,
    ) -> Result<Self, OutboxError> {
        if origin_node.is_empty() || tenant_id.is_empty() || stream_id.is_empty() {
            return Err(OutboxError::InvalidKey("identificador vazio"));
        }
        let partition = i32::try_from(partition)
            .map_err(|_| OutboxError::InvalidKey("partition acima de i32::MAX"))?;
        let authority_epoch = i64::try_from(authority_epoch)
            .ok()
            .filter(|e| *e > 0)
            .ok_or(OutboxError::InvalidKey("authority_epoch deve ser > 0"))?;
        Ok(Self {
            origin_node: origin_node.to_owned(),
            tenant_id: tenant_id.to_owned(),
            stream_id: stream_id.to_owned(),
            partition,
            authority_epoch,
        })
    }

    #[must_use]
    pub fn origin_node(&self) -> &str {
        &self.origin_node
    }

    #[must_use]
    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    #[must_use]
    pub fn stream_id(&self) -> &str {
        &self.stream_id
    }

    #[must_use]
    pub const fn partition(&self) -> i32 {
        self.partition
    }

    #[must_use]
    pub const fn authority_epoch(&self) -> i64 {
        self.authority_epoch
    }

    /// Chave textual do advisory lock por grupo.
    fn lock_key(&self) -> String {
        format!(
            "godmesh.outbox|{}|{}|{}|{}|{}",
            self.origin_node, self.tenant_id, self.stream_id, self.partition, self.authority_epoch
        )
    }
}

/// Entrada de outbox lida do banco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEntry {
    pub seq: i64,
    pub effect: PersistedEffect,
    pub attempts: i32,
    /// `published_at IS NOT NULL`: o efeito já foi reivindicado por alguém.
    pub published: bool,
}

impl PendingEntry {
    /// Efeito irreversível já reivindicado: nunca é reenviado.
    #[must_use]
    pub const fn is_irreversible(&self) -> bool {
        matches!(self.effect, PersistedEffect::Irreversible)
    }
}

/// Desfecho de uma tentativa de publicação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishOutcome {
    /// Linha reivindicada nesta chamada (`published_at` marcado). O **chamador**
    /// executa o efeito e depois chama [`PgOutbox::confirm`] ou
    /// [`PgOutbox::record_failure`].
    Published(PendingEntry),
    /// Nada publicável: não há pendência contígua neste grupo.
    NothingToPublish,
    /// Existe trabalho depois de uma sequência ausente. Nada foi publicado e
    /// nada depois da lacuna pode ser publicado.
    BlockedByGap { missing: i64 },
    /// A próxima sequência foi reivindicada por outra instância (ou por uma
    /// passagem anterior) e ainda não foi confirmada. Nenhuma linha foi tocada.
    ClaimedByAnother(PendingEntry),
    /// A próxima sequência é efeito irreversível já reivindicado e não confirmado:
    /// o resultado é desconhecido, o prefixo para aqui e **nada é reenviado**.
    AwaitingReconciliation(PendingEntry),
}

/// Leitura do prefixo contíguo de pendências.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingScan {
    /// Maior `seq` confirmado do grupo. A confirmação é contígua, então este é o
    /// prefixo confirmado.
    pub confirmed_prefix: i64,
    /// Pendências contíguas a partir de `confirmed_prefix + 1`, em ordem.
    pub entries: Vec<PendingEntry>,
    /// Primeira sequência ausente, quando a leitura parou em lacuna.
    pub missing: Option<i64>,
}

/// Registro de falha do efeito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailureRecord {
    /// Total de falhas acumuladas nesta entrada.
    pub attempts: i32,
    /// `true` quando a entrada voltou para a fila (efeito irrigável); `false`
    /// quando ela segue reivindicada e não será reenviada (irreversível).
    pub retry_scheduled: bool,
}

/// Erros do outbox persistente.
#[derive(Debug)]
pub enum OutboxError {
    /// A sequência já existe neste grupo.
    DuplicateSequence,
    /// A sequência não é `última + 1` (append fora de ordem ou salto).
    NonContiguousAppend { expected: i64, got: i64 },
    /// Não existe entrada com essa sequência no grupo.
    EntryNotFound { seq: i64 },
    /// A entrada existe, mas nunca foi publicada.
    EntryNotPublished { seq: i64 },
    /// A confirmação não é a próxima do prefixo (`prefixo + 1`).
    ConfirmationOutOfOrder { expected: i64, got: i64 },
    /// Identificador de partição inválido.
    InvalidKey(&'static str),
    /// Falha de banco.
    Storage(sqlx::Error),
}

impl fmt::Display for OutboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSequence => write!(f, "sequência duplicada"),
            Self::NonContiguousAppend { expected, got } => {
                write!(f, "append fora de ordem: esperado {expected}, recebido {got}")
            }
            Self::EntryNotFound { seq } => write!(f, "entrada inexistente: seq {seq}"),
            Self::EntryNotPublished { seq } => {
                write!(f, "entrada da seq {seq} não foi publicada; confirmar é proibido")
            }
            Self::ConfirmationOutOfOrder { expected, got } => write!(
                f,
                "confirmação fora de ordem: esperado {expected}, recebido {got}"
            ),
            Self::InvalidKey(motivo) => write!(f, "chave de partição inválida: {motivo}"),
            Self::Storage(e) => write!(f, "falha de armazenamento do outbox: {e}"),
        }
    }
}

impl std::error::Error for OutboxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for OutboxError {
    fn from(e: sqlx::Error) -> Self {
        Self::Storage(e)
    }
}

/// Destino do efeito publicado.
///
/// A publicação é uma chamada que **pode falhar**: o publisher só confirma
/// depois de `deliver` devolver `Ok`. O trait é genérico (sem `dyn`) porque o
/// futuro devolvido carrega o empréstimo de `self`.
pub trait Sink {
    /// Executa o efeito de uma entrada já reivindicada.
    fn deliver(
        &self,
        key: &PartitionKey,
        entry: &PendingEntry,
    ) -> impl Future<Output = Result<(), SinkError>> + Send;
}

/// Falha do sink. A mensagem vai para o log e para o teste; nunca para `confirmed_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkError(pub String);

impl fmt::Display for SinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SinkError {}

/// Sink de **teste** desta fase: grava a entrega em `outbox_sink`.
///
/// Não é integração externa. A PK da tabela faz o papel de detector de entrega
/// duplicada: uma segunda entrega da mesma posição falha com violação de
/// unicidade em vez de ser engolida.
#[derive(Debug, Clone)]
pub struct PgSink {
    pool: PgPool,
}

impl PgSink {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl Sink for PgSink {
    async fn deliver(
        &self,
        key: &PartitionKey,
        entry: &PendingEntry,
    ) -> Result<(), SinkError> {
        sqlx::query(
            "INSERT INTO outbox_sink \
                 (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, effect_kind) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(entry.seq)
        .bind(entry.effect.as_str())
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|e| SinkError(e.to_string()))
    }
}

/// Acesso ao outbox durável em Postgres.
#[derive(Debug, Clone)]
pub struct PgOutbox {
    pool: PgPool,
}

impl PgOutbox {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[must_use]
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Enfileira uma entrada na mesma transação do chamador quando ele passa a
    /// própria transação; aqui a transação é aberta e fechada nesta chamada.
    ///
    /// A contiguidade é verificada sob lock de grupo, então duas instâncias que
    /// anexam ao mesmo grupo não produzem duas vezes a mesma sequência nem salto.
    ///
    /// # Errors
    /// `DuplicateSequence`; `NonContiguousAppend` quando `seq != última + 1`;
    /// `InvalidKey` para `seq <= 0`; `Storage` em falha de banco.
    pub async fn append(
        &self,
        key: &PartitionKey,
        seq: i64,
        effect: PersistedEffect,
    ) -> Result<(), OutboxError> {
        if seq <= 0 {
            return Err(OutboxError::InvalidKey("seq deve ser > 0"));
        }
        let mut tx = self.pool.begin().await?;

        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(key.lock_key())
            .execute(&mut *tx)
            .await?;

        let row = sqlx::query(
            "SELECT COALESCE(MAX(seq), 0) AS last, COALESCE(bool_or(seq = $6), false) AS already \
               FROM outbox_entry \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(seq)
        .fetch_one(&mut *tx)
        .await?;

        if row.get::<bool, _>("already") {
            return Err(OutboxError::DuplicateSequence);
        }
        let last: i64 = row.get("last");
        if seq != last + 1 {
            return Err(OutboxError::NonContiguousAppend {
                expected: last + 1,
                got: seq,
            });
        }

        sqlx::query(
            "INSERT INTO outbox_entry \
                 (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, effect_kind) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(seq)
        .bind(effect.as_str())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Maior `seq` confirmado do grupo (0 quando nada foi confirmado).
    ///
    /// # Errors
    /// Falha de banco.
    pub async fn confirmed_prefix(&self, key: &PartitionKey) -> Result<i64, sqlx::Error> {
        let row = sqlx::query(
            "SELECT COALESCE(MAX(seq), 0) AS prefix \
               FROM outbox_entry \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 \
                AND confirmed_at IS NOT NULL",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .fetch_one(&self.pool)
        .await?;
        Ok(row.get::<i64, _>("prefix"))
    }

    /// Lê as pendências contíguas a partir do prefixo confirmado.
    ///
    /// A leitura para na primeira sequência ausente: o que está depois de uma
    /// lacuna não é pendente publicável, é estado inconsistente.
    ///
    /// # Errors
    /// Falha de banco.
    pub async fn pending(&self, key: &PartitionKey) -> Result<PendingScan, OutboxError> {
        let confirmed_prefix = self.confirmed_prefix(key).await?;
        let rows = sqlx::query(
            "SELECT seq, effect_kind, published_at IS NOT NULL AS published, attempts \
               FROM outbox_entry \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 \
                AND confirmed_at IS NULL AND seq > $6 \
              ORDER BY seq \
              LIMIT 1024",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(confirmed_prefix)
        .fetch_all(&self.pool)
        .await?;

        let mut entries = Vec::with_capacity(rows.len());
        let mut missing = None;
        for (expected, row) in (confirmed_prefix + 1..).zip(rows) {
            let seq: i64 = row.get("seq");
            if seq != expected {
                missing = Some(expected);
                break;
            }
            entries.push(entry_from_row(&row)?);
        }

        Ok(PendingScan {
            confirmed_prefix,
            entries,
            missing,
        })
    }

    /// Reivindica a próxima entrada publicável do grupo e marca só `published_at`.
    ///
    /// Reivindicar é um `UPDATE … RETURNING` sobre a linha escolhida com
    /// `FOR UPDATE SKIP LOCKED`: a linha fica travada dentro do comando, e uma
    /// segunda instância vê a linha já publicada (ou travada) em vez de publicá-la
    /// outra vez. Só a sequência `prefixo + 1` é candidata — lacuna bloqueia.
    ///
    /// # Errors
    /// Falha de banco.
    pub async fn publish_next(&self, key: &PartitionKey) -> Result<PublishOutcome, OutboxError> {
        let claimed = sqlx::query(
            "WITH proximo AS ( \
                 SELECT e.origin_node, e.tenant_id, e.stream_id, e.partition, \
                        e.authority_epoch, e.seq \
                   FROM outbox_entry e \
                  WHERE e.origin_node = $1 AND e.tenant_id = $2 AND e.stream_id = $3 \
                    AND e.partition = $4 AND e.authority_epoch = $5 \
                    AND e.confirmed_at IS NULL \
                    AND e.published_at IS NULL \
                    AND e.seq = (SELECT COALESCE(MAX(c.seq), 0) + 1 \
                                   FROM outbox_entry c \
                                  WHERE c.origin_node = $1 AND c.tenant_id = $2 \
                                    AND c.stream_id = $3 AND c.partition = $4 \
                                    AND c.authority_epoch = $5 \
                                    AND c.confirmed_at IS NOT NULL) \
                  ORDER BY e.seq \
                  LIMIT 1 \
                  FOR UPDATE SKIP LOCKED \
             ) \
             UPDATE outbox_entry o \
                SET published_at = now() \
               FROM proximo p \
              WHERE o.origin_node = p.origin_node AND o.tenant_id = p.tenant_id \
                AND o.stream_id = p.stream_id AND o.partition = p.partition \
                AND o.authority_epoch = p.authority_epoch AND o.seq = p.seq \
                AND o.published_at IS NULL \
             RETURNING o.seq, o.effect_kind, o.attempts",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = claimed {
            let effect_kind: String = row.get("effect_kind");
            return Ok(PublishOutcome::Published(PendingEntry {
                seq: row.get("seq"),
                effect: PersistedEffect::parse(&effect_kind)
                    .ok_or(OutboxError::InvalidKey("effect_kind fora do CHECK"))?,
                attempts: row.get("attempts"),
                // Reivindicada agora: `published_at` acabou de ser gravado.
                published: true,
            }));
        }

        self.explain_no_claim(key).await
    }

    /// Diagnóstico quando a reivindicação não trouxe linha: lacuna, disputa ou
    /// efeito irreversível aguardando reconciliação. Nada é escrito aqui.
    async fn explain_no_claim(&self, key: &PartitionKey) -> Result<PublishOutcome, OutboxError> {
        let confirmed_prefix = self.confirmed_prefix(key).await?;
        let expected = confirmed_prefix + 1;

        let next = sqlx::query(
            "SELECT seq, effect_kind, published_at IS NOT NULL AS published, attempts \
               FROM outbox_entry \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 AND seq = $6",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(expected)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = next {
            let entry = entry_from_row(&row)?;
            return Ok(if entry.is_irreversible() {
                PublishOutcome::AwaitingReconciliation(entry)
            } else {
                PublishOutcome::ClaimedByAnother(entry)
            });
        }

        let beyond = sqlx::query(
            "SELECT EXISTS ( \
                 SELECT 1 FROM outbox_entry \
                  WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                    AND partition = $4 AND authority_epoch = $5 \
                    AND confirmed_at IS NULL AND seq > $6 \
             ) AS beyond",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(expected)
        .fetch_one(&self.pool)
        .await?;

        Ok(if beyond.get::<bool, _>("beyond") {
            PublishOutcome::BlockedByGap { missing: expected }
        } else {
            PublishOutcome::NothingToPublish
        })
    }

    /// Confirma uma sequência **depois** de o efeito ter sido executado.
    ///
    /// Três recusas, todas deixando `confirmed_at` intacto: entrada inexistente,
    /// entrada não publicada (regra dura (a)) e confirmação fora de ordem (o
    /// prefixo contíguo avança um passo por vez).
    ///
    /// # Errors
    /// `EntryNotFound`, `EntryNotPublished`, `ConfirmationOutOfOrder`, `Storage`.
    pub async fn confirm(&self, key: &PartitionKey, seq: i64) -> Result<(), OutboxError> {
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query(
            "SELECT published_at IS NOT NULL AS published \
               FROM outbox_entry \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 AND seq = $6 \
              FOR UPDATE",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(seq)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(OutboxError::EntryNotFound { seq })?;

        if !row.get::<bool, _>("published") {
            return Err(OutboxError::EntryNotPublished { seq });
        }

        let prefix_row = sqlx::query(
            "SELECT COALESCE(MAX(seq), 0) AS prefix \
               FROM outbox_entry \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 \
                AND confirmed_at IS NOT NULL",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .fetch_one(&mut *tx)
        .await?;
        let expected = prefix_row.get::<i64, _>("prefix") + 1;
        if seq != expected {
            return Err(OutboxError::ConfirmationOutOfOrder { expected, got: seq });
        }

        sqlx::query(
            "UPDATE outbox_entry SET confirmed_at = now() \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 AND seq = $6",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(seq)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Registra falha do efeito: incrementa `attempts` e mantém `confirmed_at` nulo.
    ///
    /// Efeito `event` volta para a fila (`published_at = NULL`), porque reentrega
    /// de evento é permitida. Efeito `irreversible` **mantém** `published_at`: o
    /// resultado externo é desconhecido, então ele não é reenviado e fica
    /// aguardando reconciliação — o prefixo do grupo para nele.
    ///
    /// # Errors
    /// `EntryNotFound` quando não existe entrada não confirmada com essa
    /// sequência; `Storage` em falha de banco.
    pub async fn record_failure(
        &self,
        key: &PartitionKey,
        seq: i64,
    ) -> Result<FailureRecord, OutboxError> {
        let row = sqlx::query(
            "UPDATE outbox_entry \
                SET attempts = attempts + 1, \
                    published_at = CASE WHEN effect_kind = 'event' THEN NULL ELSE published_at END \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 AND seq = $6 \
                AND confirmed_at IS NULL \
             RETURNING attempts, effect_kind",
        )
        .bind(key.origin_node())
        .bind(key.tenant_id())
        .bind(key.stream_id())
        .bind(key.partition())
        .bind(key.authority_epoch())
        .bind(seq)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(OutboxError::EntryNotFound { seq })?;

        let effect_kind: String = row.get("effect_kind");
        let effect =
            PersistedEffect::parse(&effect_kind).ok_or(OutboxError::InvalidKey("effect_kind"))?;
        Ok(FailureRecord {
            attempts: row.get("attempts"),
            retry_scheduled: matches!(effect, PersistedEffect::Event),
        })
    }

    /// Grupos com trabalho pendente, em ordem determinística.
    ///
    /// # Errors
    /// Falha de banco.
    pub async fn active_partitions(&self) -> Result<Vec<PartitionKey>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT DISTINCT origin_node, tenant_id, stream_id, partition, authority_epoch \
               FROM outbox_entry \
              WHERE confirmed_at IS NULL \
              ORDER BY origin_node, tenant_id, stream_id, partition, authority_epoch",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                PartitionKey::new(
                    &row.get::<String, _>("origin_node"),
                    &row.get::<String, _>("tenant_id"),
                    &row.get::<String, _>("stream_id"),
                    u32::try_from(row.get::<i32, _>("partition")).ok()?,
                    u64::try_from(row.get::<i64, _>("authority_epoch")).ok()?,
                )
                .ok()
            })
            .collect())
    }
}

/// Converte uma linha de `outbox_entry` em [`PendingEntry`].
fn entry_from_row(row: &sqlx::postgres::PgRow) -> Result<PendingEntry, OutboxError> {
    let effect_kind: String = row.get("effect_kind");
    Ok(PendingEntry {
        seq: row.get("seq"),
        effect: PersistedEffect::parse(&effect_kind)
            .ok_or(OutboxError::InvalidKey("effect_kind fora do CHECK"))?,
        attempts: row.get("attempts"),
        published: row.get("published"),
    })
}