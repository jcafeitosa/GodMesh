//! Eventos duráveis: recebimento (`POST /v1/events`) e leitura paginada
//! (`GET /v1/events`), independentes do transporte.
//!
//! Regras desta fatia, todas vindas de contrato já existente:
//!  * a posição é a canônica da ADR-057 e vem de
//!    [`godmesh_sync::position::EventPosition`] — nenhuma tupla é montada à mão;
//!  * a admissão contra o prefixo contíguo é a regra do Inbox
//!    ([`GodmeshPosition::admit`]): só `prefixo + 1` é novidade, o resto é
//!    repetição ou lacuna, e lacuna **nunca** é saltada;
//!  * o cursor é o de [`godmesh_protocol::cursor`]: opaco, versionado e vinculado
//!    a `(principal, tenant, stream, partição, epoch, filtros)`; de outra
//!    partição/epoch é recusado, nunca traduzido.
//!
//! Fronteira de confiança: `tenant_id` e `principal` vêm do grant verificado. O
//! corpo do pedido traz só a posição, a classe de retenção e a origem; um corpo
//! que tente declarar tenancy é recusado por `deny_unknown_fields`.
//!
//! O ACK desta fatia é sempre `persisted_local`: a escrita é local e a resposta
//! nunca alega replicação, materialização ou entrega.

use godmesh_protocol::ack::AckClass;
use godmesh_protocol::cursor::{Cursor, CursorError, CursorScope, CursorTokenError};
use godmesh_sync::position::{Admission, EventPosition, PositionError};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::api::ApiError;
use crate::auth::VerifiedGrant;

/// Perfil de schema declarado no cursor. Não é negociação de wire: é a marca
/// gravada no token para que um cursor de outro perfil não seja reinterpretado.
pub const CURSOR_PROFILE: &str = "gmp.event.v1";

/// Teto de página desta fatia. É limite de implementação, não SLO nem promessa
/// de capacidade; `limit` é obrigatório justamente para não inventar default.
pub const MAX_EVENTS_LIMIT: u32 = 100;

/// Corpo do recebimento de evento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventPayload {
    pub origin_node: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    pub seq: u64,
    pub retention_class: String,
}

/// Resposta do recebimento. `ack_class` é a única alegação permitida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventAck {
    pub origin_node: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    pub seq: u64,
    pub retention_class: String,
    pub ack_class: String,
}

/// Resultado do recebimento.
///
/// `created == false` é replay idempotente da mesma posição: nada novo foi
/// gravado e a resposta repete o ACK implicado pela linha já durável. O
/// transporte traduz em 201 (criado) ou 200 (replay), nunca em erro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventOutcome {
    pub response: EventAck,
    pub created: bool,
}

/// Filtros de leitura. `limit` é obrigatório: paginação sem default publicado
/// seria parâmetro inventado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQuery {
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    pub limit: u32,
    #[serde(default)]
    pub origin_node: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Evento lido de `event_position`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    pub origin_node: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    pub seq: u64,
    pub retention_class: String,
    pub jurisdiction: String,
}

/// Página de leitura. `next_cursor` nulo significa fim do intervalo consultado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventPage {
    pub events: Vec<EventRecord>,
    pub next_cursor: Option<String>,
}

/// Persiste um evento novo, ou reconhece a repetição da mesma posição.
///
/// Ordem das verificações, deliberada: sujeito verificado → predicado do grant →
/// posição válida → jurisdição do tenant → política de retenção (mesma transação
/// da escrita) → repetição → prefixo contíguo. A política vem antes da repetição
/// para que uma classe removida falhe fechado mesmo em replay.
///
/// # Errors
/// `InvalidCredential` com sujeito verificado incompleto; `InvalidRequest` para
/// posição inválida (campo vazio, sequência zero, valores fora do tipo do banco);
/// `GrantDenied` quando o predicado não devolve `Allow`; `RetentionScopeUnresolved`
/// sem jurisdição declarada; `RetentionPolicyMissing` sem classe/cópia primária na
/// jurisdição; `SequenceGap` para salto em relação ao prefixo contíguo; `Storage`
/// para falha de banco.
pub async fn create_event(
    pool: &PgPool,
    grant: &VerifiedGrant,
    payload: EventPayload,
) -> Result<EventOutcome, ApiError> {
    grant.validate().map_err(ApiError::InvalidCredential)?;

    match grant.predicate.evaluate() {
        godmesh_identity::grant::GrantDecision::Allow => {}
        denied => return Err(ApiError::GrantDenied(denied)),
    }

    let tenant_id = &grant.subject.tenant_id;

    // A posição entra no domínio antes de qualquer efeito: origem/stream vazios e
    // sequência zero são recusados pela própria `EventPosition`.
    let position = EventPosition::new(
        &payload.origin_node,
        tenant_id,
        &payload.stream_id,
        payload.partition,
        payload.authority_epoch,
        payload.seq,
    )
    .map_err(invalid_position)?;

    let partition = i32::try_from(position.partition())
        .map_err(|_| ApiError::InvalidRequest("partition fora do intervalo".to_owned()))?;
    let authority_epoch = i64::try_from(position.authority_epoch())
        .map_err(|_| ApiError::InvalidRequest("authority_epoch fora do intervalo".to_owned()))?;
    let seq = i64::try_from(position.seq())
        .map_err(|_| ApiError::InvalidRequest("seq fora do intervalo".to_owned()))?;

    let mut tx = pool.begin().await.map_err(ApiError::Storage)?;
    let jurisdiction = resolve_jurisdiction(&mut tx, tenant_id).await?;

    // Política efetiva da jurisdição do tenant, sob o mesmo lock que a escrita.
    let class = resolve_retention_class(&mut tx, &payload.retention_class, &jurisdiction).await?;

    // Repetição da mesma tupla: idempotente, sem segundo INSERT. A classe da
    // resposta é a que já está durável, não a declarada no replay.
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT retention_class FROM event_position \
          WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
            AND partition = $4 AND authority_epoch = $5 AND seq = $6",
    )
    .bind(&payload.origin_node)
    .bind(tenant_id)
    .bind(&payload.stream_id)
    .bind(partition)
    .bind(authority_epoch)
    .bind(seq)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;

    if let Some(stored_class) = stored {
        let response = ack_for(&payload, &stored_class, position.seq());
        tx.rollback().await.map_err(ApiError::Storage)?;
        return Ok(EventOutcome {
            response,
            created: false,
        });
    }

    let prefix = contiguous_prefix(
        &mut tx,
        &payload.origin_node,
        tenant_id,
        &payload.stream_id,
        partition,
        authority_epoch,
    )
    .await?;

    match position.admit(prefix) {
        Admission::Next => {}
        Admission::Duplicate => {
            // Inalcançável depois da checagem acima, mas a exaustividade do
            // `match` obriga: se aparecer, é repetição, não lacuna.
            tx.rollback().await.map_err(ApiError::Storage)?;
            return Ok(EventOutcome {
                response: ack_for(&payload, &class, position.seq()),
                created: false,
            });
        }
        Admission::Gap { expected } => {
            tx.rollback().await.map_err(ApiError::Storage)?;
            return Err(ApiError::SequenceGap {
                expected,
                got: position.seq(),
            });
        }
    }

    // `ON CONFLICT DO NOTHING` cobre a corrida entre duas conexões que passaram
    // pela checagem: quem perde não duplica, vira replay.
    let inserted = sqlx::query(
        "INSERT INTO event_position \
             (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, \
              retention_class, jurisdiction) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (origin_node, tenant_id, stream_id, partition, authority_epoch, seq) \
         DO NOTHING",
    )
    .bind(&payload.origin_node)
    .bind(tenant_id)
    .bind(&payload.stream_id)
    .bind(partition)
    .bind(authority_epoch)
    .bind(seq)
    .bind(&class)
    .bind(&jurisdiction)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;

    let created = inserted.rows_affected() == 1;
    if !created {
        tx.rollback().await.map_err(ApiError::Storage)?;
        return Ok(EventOutcome {
            response: ack_for(&payload, &class, position.seq()),
            created: false,
        });
    }

    tx.commit().await.map_err(ApiError::Storage)?;
    Ok(EventOutcome {
        response: ack_for(&payload, &class, position.seq()),
        created: true,
    })
}

/// Lê uma página de eventos do escopo autorizado.
///
/// O cursor carrega a posição de retomada e o vínculo; a query sempre filtra por
/// `tenant_id` do grant. Cursor de outra partição/epoch/tenant/filtro é recusado
/// (`cursor_incomparable`), cursor ilegível ou de outro principal é recusado
/// (`cursor_invalid`) e cursor ausente significa começar do início da stream.
///
/// # Errors
/// `InvalidCredential`/`GrantDenied` como no recebimento; `InvalidRequest` para
/// `limit` fora de `1..=MAX_EVENTS_LIMIT` ou escopo inválido; `CursorInvalid`,
/// `CursorIncomparable`; `RetentionScopeUnresolved` sem jurisdição declarada;
/// `Storage` para falha de banco.
pub async fn list_events(
    pool: &PgPool,
    grant: &VerifiedGrant,
    query: EventQuery,
) -> Result<EventPage, ApiError> {
    grant.validate().map_err(ApiError::InvalidCredential)?;

    match grant.predicate.evaluate() {
        godmesh_identity::grant::GrantDecision::Allow => {}
        denied => return Err(ApiError::GrantDenied(denied)),
    }

    if query.limit == 0 || query.limit > MAX_EVENTS_LIMIT {
        return Err(ApiError::InvalidRequest(format!(
            "limit precisa estar entre 1 e {MAX_EVENTS_LIMIT}"
        )));
    }

    let tenant_id = &grant.subject.tenant_id;
    let scope = CursorScope::new(
        tenant_id,
        &query.stream_id,
        query.partition,
        query.authority_epoch,
    )
    .map_err(|e| ApiError::InvalidRequest(format!("escopo de cursor inválido: {e:?}")))?;

    let filters = filters_for(query.origin_node.as_deref());

    let mut tx = pool.begin().await.map_err(ApiError::Storage)?;

    // A jurisdição entra no cursor como escopo de retenção efetiva: um cursor
    // emitido sob outra residência não serve a esta consulta.
    let jurisdiction = resolve_jurisdiction(&mut tx, tenant_id).await?;

    let mut cursor = Cursor::new(
        scope.clone(),
        &grant.subject.principal,
        &filters,
        CURSOR_PROFILE,
        &jurisdiction,
    )
    .map_err(|e| ApiError::InvalidRequest(format!("escopo de cursor inválido: {e:?}")))?;

    let mut after: i64 = 0;
    if let Some(raw) = query.cursor.as_deref() {
        let decoded = Cursor::decode(raw).map_err(cursor_token_error)?;
        decoded
            .cursor()
            .check_against(&scope)
            .map_err(cursor_error)?;
        decoded
            .cursor()
            .check_binding(&grant.subject.principal, &filters)
            .map_err(cursor_error)?;
        cursor = decoded.cursor().clone();
        after = i64::try_from(decoded.last_seq())
            .map_err(|_| ApiError::InvalidRequest("cursor fora do intervalo".to_owned()))?;
    }

    let partition = i32::try_from(query.partition)
        .map_err(|_| ApiError::InvalidRequest("partition fora do intervalo".to_owned()))?;
    let authority_epoch = i64::try_from(query.authority_epoch)
        .map_err(|_| ApiError::InvalidRequest("authority_epoch fora do intervalo".to_owned()))?;
    let fetch = i64::from(query.limit) + 1; // +1 só para saber se há próxima página

    let rows: Vec<(String, String, i32, i64, i64, String, String)> = sqlx::query_as(
        "SELECT origin_node, stream_id, partition, authority_epoch, seq, retention_class, jurisdiction \
           FROM event_position \
          WHERE tenant_id = $1 AND stream_id = $2 AND partition = $3 AND authority_epoch = $4 \
            AND ($5::text IS NULL OR origin_node = $5) \
            AND seq > $6 \
          ORDER BY origin_node, seq \
          LIMIT $7",
    )
    .bind(tenant_id)
    .bind(&query.stream_id)
    .bind(partition)
    .bind(authority_epoch)
    .bind(query.origin_node.as_deref())
    .bind(after)
    .bind(fetch)
    .fetch_all(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;

    tx.commit().await.map_err(ApiError::Storage)?;

    let has_more = rows.len() > query.limit as usize;
    let mut events = Vec::with_capacity(rows.len());
    for row in rows.iter().take(query.limit as usize) {
        events.push(record_from_row(row)?);
    }
    let next_cursor = if has_more {
        let last = events
            .last()
            .map(|e| e.seq)
            .ok_or_else(|| ApiError::Storage(sqlx::Error::Protocol("página vazia".to_owned())))?;
        Some(cursor.encode(last))
    } else {
        None
    };

    Ok(EventPage {
        events,
        next_cursor,
    })
}

/// Converte a linha persistida no tipo de resposta, sem completar nada por
/// inferência: valor fora do intervalo é falha de armazenamento, não correção.
fn record_from_row(
    row: &(String, String, i32, i64, i64, String, String),
) -> Result<EventRecord, ApiError> {
    let (origin_node, stream_id, partition, authority_epoch, seq, retention_class, jurisdiction) =
        row;
    Ok(EventRecord {
        origin_node: origin_node.clone(),
        stream_id: stream_id.clone(),
        partition: u32::try_from(*partition)
            .map_err(|_| storage_invalid("partition persistida fora do intervalo"))?,
        authority_epoch: u64::try_from(*authority_epoch)
            .map_err(|_| storage_invalid("authority_epoch persistido fora do intervalo"))?,
        seq: u64::try_from(*seq)
            .map_err(|_| storage_invalid("seq persistida fora do intervalo"))?,
        retention_class: retention_class.clone(),
        jurisdiction: jurisdiction.clone(),
    })
}

fn storage_invalid(detail: &str) -> ApiError {
    ApiError::Storage(sqlx::Error::Protocol(detail.to_owned()))
}

/// Filtro de leitura em forma estável: entra no token e precisa ser idêntico na
/// retomada, senão o cursor é de outra consulta.
fn filters_for(origin_node: Option<&str>) -> String {
    match origin_node {
        Some(node) => format!("origin_node={node}"),
        None => "origin_node=*".to_owned(),
    }
}

/// ACK do recebimento. Nesta fatia a única evidência é a linha durável local.
fn ack_for(payload: &EventPayload, retention_class: &str, seq: u64) -> EventAck {
    EventAck {
        origin_node: payload.origin_node.clone(),
        stream_id: payload.stream_id.clone(),
        partition: payload.partition,
        authority_epoch: payload.authority_epoch,
        seq,
        retention_class: retention_class.to_owned(),
        ack_class: AckClass::PersistedLocal.as_str().to_owned(),
    }
}

fn invalid_position(error: PositionError) -> ApiError {
    let detail = match error {
        PositionError::EmptyField => "origem, stream e tenant precisam ser não vazios",
        PositionError::ZeroSequence => "seq precisa ser positiva",
    };
    ApiError::InvalidRequest(detail.to_owned())
}

/// Jurisdição declarada do tenant autenticado (ADR-066). Ausência de linha falha
/// fechado; o corpo do pedido nunca escolhe jurisdição.
pub(crate) async fn resolve_jurisdiction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant_id: &str,
) -> Result<String, ApiError> {
    let jurisdiction: Option<String> =
        sqlx::query_scalar("SELECT jurisdiction FROM tenant_region WHERE tenant_id = $1 FOR SHARE")
            .bind(tenant_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(ApiError::Storage)?;
    jurisdiction.ok_or(ApiError::RetentionScopeUnresolved)
}

/// Classe de retenção **com cópia primária** na jurisdição do tenant. Sem classe
/// ou sem cópia autorizada, nada é persistido (ADR-062). A FK de `event_position`
/// fecha a corrida com uma remoção posterior.
pub(crate) async fn resolve_retention_class(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    class: &str,
    jurisdiction: &str,
) -> Result<String, ApiError> {
    let found: Option<String> = sqlx::query_scalar(
        "SELECT rc.class \
           FROM retention_class rc \
           JOIN retention_copy cp \
             ON cp.class = rc.class \
            AND cp.jurisdiction = rc.jurisdiction \
            AND cp.copy_scope = 'primary' \
          WHERE rc.class = $1 \
            AND rc.jurisdiction = $2 \
          FOR SHARE OF rc, cp",
    )
    .bind(class)
    .bind(jurisdiction)
    .fetch_optional(&mut **tx)
    .await
    .map_err(ApiError::Storage)?;
    found.ok_or(ApiError::RetentionPolicyMissing)
}

/// Maior `p` tal que todo `1..=p` existe neste namespace — o prefixo contíguo.
///
/// O menor buraco da sequência define o prefixo: `primeiro_ausente - 1`, ou o
/// máximo quando não há buraco. Não é `max(seq)`: um buraco deixado por escrita
/// fora desta API não pode fazer a API aceitar o que vem depois dele.
pub(crate) async fn contiguous_prefix(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    origin_node: &str,
    tenant_id: &str,
    stream_id: &str,
    partition: i32,
    authority_epoch: i64,
) -> Result<u64, ApiError> {
    let prefix: i64 = sqlx::query_scalar(
        "WITH scope AS ( \
             SELECT seq FROM event_position \
              WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
                AND partition = $4 AND authority_epoch = $5 \
         ), missing AS ( \
             SELECT g AS seq \
               FROM generate_series(1, COALESCE((SELECT MAX(seq) FROM scope), 0)) AS g \
              WHERE NOT EXISTS (SELECT 1 FROM scope s WHERE s.seq = g) \
         ) \
         SELECT COALESCE( \
                  (SELECT MIN(seq) - 1 FROM missing), \
                  (SELECT COALESCE(MAX(seq), 0) FROM scope) \
                )::bigint",
    )
    .bind(origin_node)
    .bind(tenant_id)
    .bind(stream_id)
    .bind(partition)
    .bind(authority_epoch)
    .fetch_one(&mut **tx)
    .await
    .map_err(ApiError::Storage)?;
    u64::try_from(prefix).map_err(|_| storage_invalid("prefixo contíguo negativo"))
}

/// Cursor de outra partição/epoch/tenant/filtro é **incomparável** (400); de outro
/// principal ou sem partição é **inválido** (400). Causas distintas, códigos
/// distintos: nenhuma delas é "cursor vazio".
pub(crate) fn cursor_error(error: CursorError) -> ApiError {
    match error {
        CursorError::PartitionMismatch => {
            ApiError::CursorIncomparable("cursor é de outra partição".to_owned())
        }
        CursorError::EpochIncomparable => {
            ApiError::CursorIncomparable("cursor é de epoch incomparável".to_owned())
        }
        CursorError::TenantMismatch => {
            ApiError::CursorIncomparable("cursor é de outro tenant ou stream".to_owned())
        }
        CursorError::FilterMismatch => {
            ApiError::CursorIncomparable("cursor é de outro filtro".to_owned())
        }
        CursorError::PrincipalMismatch => {
            ApiError::CursorInvalid("cursor é de outro principal".to_owned())
        }
        CursorError::PartitionRequired => {
            ApiError::CursorInvalid("cursor sem partição é inválido".to_owned())
        }
        CursorError::EmptyField => {
            ApiError::CursorInvalid("cursor com campo obrigatório vazio".to_owned())
        }
    }
}

pub(crate) fn cursor_token_error(error: CursorTokenError) -> ApiError {
    let detail = match error {
        CursorTokenError::Malformed => "cursor ilegível".to_owned(),
        CursorTokenError::UnsupportedVersion(v) => {
            format!("versão de cursor não suportada: v{v}")
        }
        CursorTokenError::PartitionRequired => "cursor sem partição é inválido".to_owned(),
        CursorTokenError::EmptyField => "cursor com campo obrigatório vazio".to_owned(),
    };
    ApiError::CursorInvalid(detail)
}
