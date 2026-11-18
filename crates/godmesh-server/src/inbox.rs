//! Inbox de sync no servidor: `POST /v1/sync/inbox` e `GET /v1/sync/state`.
//!
//! Esta fatia expõe o Inbox durável da ADR-057/061 sobre o banco:
//!
//!  * a decisão de admissão **não** é reimplementada aqui: quem decide
//!    aceitação, repetição e lacuna é [`godmesh_sync::inbox::Inbox`], retomado do
//!    prefixo contíguo já durável;
//!  * o prefixo contíguo persistido (`sync_inbox_state`, migração 0003) é
//!    atualizado na **mesma transação** que insere as posições aceitas em
//!    `event_position`: ou o lote contíguo e o checkpoint sobrevivem juntos, ou
//!    nenhum dos dois;
//!  * lacuna não é saltada: o prefixo aceito é gravado, tudo a partir da lacuna
//!    é recusado com `409 sequence_gap` nomeando `expected` e `received`;
//!  * `ack_class` da resposta é, no máximo, `persisted_local` — a evidência é a
//!    linha durável local, nunca replicação, materialização ou entrega.
//!
//! Fronteira de confiança: `tenant_id` e `principal` vêm do grant verificado. O
//! corpo do lote traz posições, não tenancy; `deny_unknown_fields` recusa um
//! corpo que tente declarar `tenant_id`.
//!
//! Parâmetros desta fatia (não são SLO nem capacidade publicada):
//!  * lote máximo de **100 posições** ([`MAX_INBOX_BATCH`]); acima disso o
//!    pedido é recusado com `422 invalid_request` em vez de fatiado em silêncio;
//!  * o lote é de **um único namespace** (origem, stream, partição, epoch). Um
//!    lote com duas partições/epochs/streams/origens diferentes é recusado com
//!    `400 cursor_incomparable`, porque `contiguous_prefix` e `accepted` não
//!    teriam um namespace único a que se referir;
//!  * a ordem do lote é a ordem de admissão. Posição fora de ordem produz lacuna;
//!    o servidor não reordena o lote.

use godmesh_protocol::ack::AckClass;
use godmesh_sync::inbox::{Inbox, InboxError, InboxOutcome};
use godmesh_sync::position::{EventPosition, PositionError};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::api::ApiError;
use crate::auth::VerifiedGrant;
use crate::events::{contiguous_prefix, resolve_retention_class, resolve_jurisdiction};

/// Teto de posições por lote. Limite desta fatia, declarado e recusado — nunca
/// truncado em silêncio.
pub const MAX_INBOX_BATCH: usize = 100;

/// Posição recebida no lote. Espelha a posição canônica da ADR-057 **sem**
/// `tenant_id`: tenancy vem do grant, nunca do corpo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InboxPositionPayload {
    pub origin_node: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    pub seq: u64,
}

/// Corpo do lote.
///
/// `retention_class` é obrigatória no envelope (e não em cada posição): toda
/// posição aceita vira linha em `event_position`, que não existe sem classe com
/// política na jurisdição do tenant (ADR-062).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InboxBatchPayload {
    pub retention_class: String,
    pub positions: Vec<InboxPositionPayload>,
}

/// Resposta do lote aceito.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxBatchResponse {
    pub accepted: u64,
    pub duplicates: u64,
    pub contiguous_prefix: u64,
    pub ack_class: String,
}

/// Filtros de `GET /v1/sync/state`.
///
/// `origin_node` é opcional: sem ele, o estado é o agregado conservador do
/// tenant+stream+partição+epoch (prefixo mínimo entre origens). Com ele, o
/// estado é exatamente do namespace `(tenant, origem, stream, partição, epoch)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InboxStateQuery {
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    #[serde(default)]
    pub origin_node: Option<String>,
}

/// Estado do Inbox de um namespace.
///
/// `contiguous_prefix == 0` e `last_recorded_at == null` significam "nenhum
/// evento durável neste namespace" — stream desconhecida é estado vazio, não
/// erro (não existe lista de streams autorizadas a consultar por tentativa).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxState {
    pub contiguous_prefix: u64,
    pub last_recorded_at: Option<String>,
    pub gaps: Vec<InboxGap>,
}

/// Intervalo ausente do namespace, inclusivo nas duas pontas.
///
/// `gaps` é derivado das posições duráveis, não da memória do processo: um
/// buraco em `event_position` aparece aqui mesmo depois de reinício. Quando o
/// Inbox está contíguo (invariante desta API), a lista é vazia — a autoridade
/// sobre "existe lacuna" continua sendo o `409 sequence_gap`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxGap {
    pub origin_node: String,
    pub from: u64,
    pub to: u64,
}

/// Recebe um lote de posições e avança o prefixo contíguo do namespace.
///
/// Ordem deliberada das verificações: sujeito verificado → predicado do grant →
/// formato do lote (tamanho, namespace único, posições válidas) → jurisdição e
/// política de retenção → prefixo durável → admissão pelo Inbox. Nada é gravado
/// antes de todas as recusas de fronteira.
///
/// # Errors
/// `InvalidCredential`/`GrantDenied` como nas demais rotas; `InvalidRequest`
/// para lote vazio, acima de [`MAX_INBOX_BATCH`] ou posição inválida;
/// `CursorIncomparable` para posição de outra partição/epoch/stream/origem que
/// não o namespace do lote; `RetentionScopeUnresolved`/`RetentionPolicyMissing`
/// sem jurisdição ou política; `InboxGap` (409) quando o lote salta posição — o
/// prefixo aceito até a lacuna é gravado e reportado; `Storage` para falha de
/// banco.
pub async fn receive_batch(
    pool: &PgPool,
    grant: &VerifiedGrant,
    payload: InboxBatchPayload,
) -> Result<InboxBatchResponse, ApiError> {
    grant.validate().map_err(ApiError::InvalidCredential)?;

    match grant.predicate.evaluate() {
        godmesh_identity::grant::GrantDecision::Allow => {}
        denied => return Err(ApiError::GrantDenied(denied)),
    }

    let tenant_id = &grant.subject.tenant_id;

    if payload.positions.is_empty() {
        return Err(ApiError::InvalidRequest(
            "lote vazio: nada a aceitar e nada a confirmar".to_owned(),
        ));
    }
    if payload.positions.len() > MAX_INBOX_BATCH {
        return Err(ApiError::InvalidRequest(format!(
            "lote com {} posições excede o teto de {MAX_INBOX_BATCH} por pedido",
            payload.positions.len()
        )));
    }

    // Namespace do lote: o primeiro item declara, os demais precisam concordar.
    // Um lote multipartição não tem um prefixo único a devolver, então é recusado
    // em vez de respondido pela média.
    let anchor = &payload.positions[0];
    for position in &payload.positions {
        check_same_namespace(anchor, position)?;
    }

    // A posição entra no domínio antes de qualquer efeito: origem/stream vazios e
    // sequência zero são recusados pela própria `EventPosition`.
    let mut positions = Vec::with_capacity(payload.positions.len());
    for item in &payload.positions {
        positions.push(
            EventPosition::new(
                &item.origin_node,
                tenant_id,
                &item.stream_id,
                item.partition,
                item.authority_epoch,
                item.seq,
            )
            .map_err(invalid_position)?,
        );
    }

    let partition = i32::try_from(anchor.partition)
        .map_err(|_| ApiError::InvalidRequest("partition fora do intervalo".to_owned()))?;
    let authority_epoch = i64::try_from(anchor.authority_epoch)
        .map_err(|_| ApiError::InvalidRequest("authority_epoch fora do intervalo".to_owned()))?;

    let mut tx = pool.begin().await.map_err(ApiError::Storage)?;
    let jurisdiction = resolve_jurisdiction(&mut tx, tenant_id).await?;
    let class = resolve_retention_class(&mut tx, &payload.retention_class, &jurisdiction).await?;

    // Prefixo de retomada: o checkpoint persistido e o prefixo derivado das
    // posições duráveis, o maior dos dois. `POST /v1/events` também grava em
    // `event_position`; derivar evita que uma escrita daquela rota deixe o Inbox
    // tratando posição já durável como novidade.
    let stored = stored_prefix(
        &mut tx,
        tenant_id,
        &anchor.origin_node,
        &anchor.stream_id,
        partition,
        authority_epoch,
    )
    .await?;
    let derived = contiguous_prefix(
        &mut tx,
        &anchor.origin_node,
        tenant_id,
        &anchor.stream_id,
        partition,
        authority_epoch,
    )
    .await?;
    let start = stored.max(derived);

    let mut inbox = Inbox::resuming(
        &anchor.origin_node,
        tenant_id,
        &anchor.stream_id,
        anchor.partition,
        anchor.authority_epoch,
        start,
    );

    let mut accepted: u64 = 0;
    let mut duplicates: u64 = 0;
    let mut gap: Option<(u64, u64)> = None;

    for position in positions {
        let received = position.seq();
        match inbox.accept(position) {
            Ok(InboxOutcome::Accepted { .. }) => {
                // `ON CONFLICT DO NOTHING` cobre a corrida com `POST /v1/events`:
                // se a posição já está durável, nada é reescrito e ela conta como
                // repetição, não como aceitação nova.
                let inserted = sqlx::query(
                    "INSERT INTO event_position \
                         (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, \
                          retention_class, jurisdiction) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                     ON CONFLICT (origin_node, tenant_id, stream_id, partition, authority_epoch, seq) \
                     DO NOTHING",
                )
                .bind(&anchor.origin_node)
                .bind(tenant_id)
                .bind(&anchor.stream_id)
                .bind(partition)
                .bind(authority_epoch)
                .bind(i64::try_from(received).map_err(|_| {
                    ApiError::InvalidRequest("seq fora do intervalo".to_owned())
                })?)
                .bind(&class)
                .bind(&jurisdiction)
                .execute(&mut *tx)
                .await
                .map_err(ApiError::Storage)?;
                if inserted.rows_affected() == 1 {
                    accepted += 1;
                } else {
                    duplicates += 1;
                }
            }
            Ok(InboxOutcome::Duplicate) => duplicates += 1,
            // Lacuna: nada além do prefixo é aceito. O que veio antes da lacuna
            // já foi aceito e segue para o commit; o resto do lote é descartado.
            Ok(InboxOutcome::Gap { expected }) => {
                gap = Some((expected, received));
                break;
            }
            // Pré-validado acima; mapear é obrigatório para não deixar o erro do
            // domínio virar 500 se a validação de fronteira mudar.
            Err(e) => {
                tx.rollback().await.map_err(ApiError::Storage)?;
                return Err(incomparable(e));
            }
        }
    }

    let contiguous = inbox.contiguous_prefix();

    // Prefixo e posições aceitas na **mesma** transação (ADR-057/061). Sem
    // evento aceito não há o que avançar: o checkpoint não é reescrito.
    if accepted > 0 {
        sqlx::query(
            "INSERT INTO sync_inbox_state \
                 (tenant_id, origin_node, stream_id, partition, authority_epoch, \
                  contiguous_prefix, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, now()) \
             ON CONFLICT (tenant_id, origin_node, stream_id, partition, authority_epoch) \
             DO UPDATE SET contiguous_prefix = GREATEST( \
                                sync_inbox_state.contiguous_prefix, EXCLUDED.contiguous_prefix), \
                           updated_at = now()",
        )
        .bind(tenant_id)
        .bind(&anchor.origin_node)
        .bind(&anchor.stream_id)
        .bind(partition)
        .bind(authority_epoch)
        .bind(i64::try_from(contiguous).map_err(|_| {
            ApiError::Storage(sqlx::Error::Protocol("prefixo fora do intervalo".to_owned()))
        })?)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Storage)?;
    }

    tx.commit().await.map_err(ApiError::Storage)?;

    if let Some((expected, received)) = gap {
        return Err(ApiError::InboxGap {
            expected,
            received,
            accepted,
            duplicates,
            contiguous_prefix: contiguous,
        });
    }

    Ok(InboxBatchResponse {
        accepted,
        duplicates,
        contiguous_prefix: contiguous,
        ack_class: AckClass::PersistedLocal.as_str().to_owned(),
    })
}

/// Lê o estado do Inbox do escopo autorizado.
///
/// Sem `origin_node`, agrega as origens do namespace: o prefixo é o **mínimo**
/// entre elas (alegar o máximo confirmaria, por uma origem, posições que outra
/// não tem) e `gaps` lista os intervalos ausentes de cada origem.
///
/// # Errors
/// `InvalidCredential`/`GrantDenied` como nas demais rotas; `InvalidRequest` para
/// escopo inválido; `RetentionScopeUnresolved` sem jurisdição declarada (o
/// escopo de retenção do tenant é pré-condição de consulta, como na leitura de
/// eventos); `Storage` para falha de banco.
pub async fn read_state(
    pool: &PgPool,
    grant: &VerifiedGrant,
    query: InboxStateQuery,
) -> Result<InboxState, ApiError> {
    grant.validate().map_err(ApiError::InvalidCredential)?;

    match grant.predicate.evaluate() {
        godmesh_identity::grant::GrantDecision::Allow => {}
        denied => return Err(ApiError::GrantDenied(denied)),
    }

    if query.stream_id.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "stream_id vazio não identifica stream".to_owned(),
        ));
    }
    if query.origin_node.as_deref().is_some_and(|o| o.trim().is_empty()) {
        return Err(ApiError::InvalidRequest(
            "origin_node vazio não identifica origem".to_owned(),
        ));
    }

    let tenant_id = &grant.subject.tenant_id;
    let partition = i32::try_from(query.partition)
        .map_err(|_| ApiError::InvalidRequest("partition fora do intervalo".to_owned()))?;
    let authority_epoch = i64::try_from(query.authority_epoch)
        .map_err(|_| ApiError::InvalidRequest("authority_epoch fora do intervalo".to_owned()))?;

    let mut tx = pool.begin().await.map_err(ApiError::Storage)?;
    let _jurisdiction = resolve_jurisdiction(&mut tx, tenant_id).await?;

    let origins = namespace_origins(
        &mut tx,
        tenant_id,
        &query.stream_id,
        partition,
        authority_epoch,
        query.origin_node.as_deref(),
    )
    .await?;

    let mut contiguous: Option<u64> = None;
    let mut last_recorded_at: Option<String> = None;
    let mut gaps = Vec::new();

    for origin in &origins {
        let stored = stored_prefix(
            &mut tx,
            tenant_id,
            origin,
            &query.stream_id,
            partition,
            authority_epoch,
        )
        .await?;
        let derived = contiguous_prefix(
            &mut tx,
            origin,
            tenant_id,
            &query.stream_id,
            partition,
            authority_epoch,
        )
        .await?;
        let prefix = stored.max(derived);
        contiguous = Some(contiguous.map_or(prefix, |current: u64| current.min(prefix)));

        let updated: Option<String> = sqlx::query_scalar(
            "SELECT to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') \
               FROM sync_inbox_state \
              WHERE tenant_id = $1 AND origin_node = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5",
        )
        .bind(tenant_id)
        .bind(origin)
        .bind(&query.stream_id)
        .bind(partition)
        .bind(authority_epoch)
        .fetch_optional(&mut *tx)
        .await
        .map_err(ApiError::Storage)?;
        if let Some(updated) = updated {
            last_recorded_at = Some(match last_recorded_at {
                Some(current) if current >= updated => current,
                _ => updated,
            });
        }

        gaps.extend(
            namespace_gaps(
                &mut tx,
                origin,
                tenant_id,
                &query.stream_id,
                partition,
                authority_epoch,
            )
            .await?,
        );
    }

    tx.commit().await.map_err(ApiError::Storage)?;

    Ok(InboxState {
        contiguous_prefix: contiguous.unwrap_or(0),
        last_recorded_at,
        gaps,
    })
}

/// Recusa lote que mistura namespaces. Partição/epoch/stream/origem diferentes do
/// namespace declarado pelo primeiro item tornam a posição **incomparável**: ela
/// não pertence ao Inbox a que o prefixo da resposta se refere.
fn check_same_namespace(
    anchor: &InboxPositionPayload,
    other: &InboxPositionPayload,
) -> Result<(), ApiError> {
    let divergences: [(&str, bool); 4] = [
        ("origin_node", anchor.origin_node != other.origin_node),
        ("stream_id", anchor.stream_id != other.stream_id),
        ("partition", anchor.partition != other.partition),
        ("authority_epoch", anchor.authority_epoch != other.authority_epoch),
    ];
    let Some((field, _)) = divergences.iter().find(|(_, diverges)| *diverges) else {
        return Ok(());
    };
    Err(ApiError::CursorIncomparable(format!(
        "posição fora do namespace do lote: {field} difere do namespace declarado \
         (origem {}, stream {}, partição {}, epoch {})",
        anchor.origin_node, anchor.stream_id, anchor.partition, anchor.authority_epoch
    )))
}

fn invalid_position(error: PositionError) -> ApiError {
    let detail = match error {
        PositionError::EmptyField => "origem, stream e tenant precisam ser não vazios",
        PositionError::ZeroSequence => "seq precisa ser positiva",
    };
    ApiError::InvalidRequest(detail.to_owned())
}

/// Erro do domínio traduzido para o código de transporte. Os dois casos são de
/// comparabilidade de posição, e o handler já pré-valida o namespace: se um deles
/// aparecer, veio de dado inconsistente e não de caminho feliz.
fn incomparable(error: InboxError) -> ApiError {
    let detail = match error {
        InboxError::PartitionMismatch => "posição é de outra partição",
        InboxError::EpochIncomparable => "posição é de epoch incomparável",
    };
    ApiError::CursorIncomparable(detail.to_owned())
}

/// Prefixo contíguo gravado como checkpoint do namespace (`0` sem linha).
async fn stored_prefix(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &str,
    origin_node: &str,
    stream_id: &str,
    partition: i32,
    authority_epoch: i64,
) -> Result<u64, ApiError> {
    let stored: Option<i64> = sqlx::query_scalar(
        "SELECT contiguous_prefix FROM sync_inbox_state \
          WHERE tenant_id = $1 AND origin_node = $2 AND stream_id = $3 \
            AND partition = $4 AND authority_epoch = $5",
    )
    .bind(tenant_id)
    .bind(origin_node)
    .bind(stream_id)
    .bind(partition)
    .bind(authority_epoch)
    .fetch_optional(&mut **tx)
    .await
    .map_err(ApiError::Storage)?;
    match stored {
        None => Ok(0),
        Some(value) => u64::try_from(value).map_err(|_| {
            ApiError::Storage(sqlx::Error::Protocol(
                "checkpoint de Inbox negativo no banco".to_owned(),
            ))
        }),
    }
}

/// Origens com estado durável no namespace consultado. Vazio significa stream
/// desconhecida — estado zero, não erro.
async fn namespace_origins(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &str,
    stream_id: &str,
    partition: i32,
    authority_epoch: i64,
    origin_node: Option<&str>,
) -> Result<Vec<String>, ApiError> {
    let origins: Vec<String> = sqlx::query_scalar(
        "SELECT origin_node FROM event_position \
          WHERE tenant_id = $1 AND stream_id = $2 AND partition = $3 AND authority_epoch = $4 \
            AND ($5::text IS NULL OR origin_node = $5) \
         UNION \
         SELECT origin_node FROM sync_inbox_state \
          WHERE tenant_id = $1 AND stream_id = $2 AND partition = $3 AND authority_epoch = $4 \
            AND ($5::text IS NULL OR origin_node = $5) \
         ORDER BY 1",
    )
    .bind(tenant_id)
    .bind(stream_id)
    .bind(partition)
    .bind(authority_epoch)
    .bind(origin_node)
    .fetch_all(&mut **tx)
    .await
    .map_err(ApiError::Storage)?;
    Ok(origins)
}

/// Intervalos ausentes entre `1` e a maior posição durável da origem, agrupados.
///
/// Derivado das linhas existentes: não depende de memória do processo e não
/// inventa lacuna onde a retenção já não guarda nada — se `event_position` está
/// vazio, a lista é vazia.
async fn namespace_gaps(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    origin_node: &str,
    tenant_id: &str,
    stream_id: &str,
    partition: i32,
    authority_epoch: i64,
) -> Result<Vec<InboxGap>, ApiError> {
    let missing: Vec<i64> = sqlx::query_scalar(
        "WITH scope AS ( \
             SELECT seq FROM event_position \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 \
         ), missing AS ( \
             SELECT g AS seq \
               FROM generate_series(1, COALESCE((SELECT MAX(seq) FROM scope), 0)) AS g \
              WHERE NOT EXISTS (SELECT 1 FROM scope s WHERE s.seq = g) \
         ) \
         SELECT seq FROM missing ORDER BY seq",
    )
    .bind(origin_node)
    .bind(tenant_id)
    .bind(stream_id)
    .bind(partition)
    .bind(authority_epoch)
    .fetch_all(&mut **tx)
    .await
    .map_err(ApiError::Storage)?;

    let mut gaps: Vec<InboxGap> = Vec::new();
    for seq in missing {
        let seq = u64::try_from(seq).map_err(|_| {
            ApiError::Storage(sqlx::Error::Protocol("seq negativa no banco".to_owned()))
        })?;
        match gaps.last_mut() {
            // Sequências ausentes consecutivas viram um intervalo só.
            Some(last) if last.to.checked_add(1) == Some(seq) => last.to = seq,
            _ => gaps.push(InboxGap {
                origin_node: origin_node.to_owned(),
                from: seq,
                to: seq,
            }),
        }
    }
    Ok(gaps)
}