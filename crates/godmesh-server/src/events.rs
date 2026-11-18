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
//! Fronteira de confiança, em duas camadas (S-sec-2 — regra dura de
//! `docs/04-seguranca/abuso-e-malha-maligna.md` §5):
//!
//! 1. o **grant** autenticou o *cliente* (um token com `event:write`); ele não
//!    prova nada sobre a origem do evento;
//! 2. a **assinatura do nó** prova o que o cliente afirma: posição canônica,
//!    `tenant_id`, classe de retenção, instante de emissão e — decisivamente —
//!    que a origem declarada é o próprio nó que assinou. Sem envelope válido,
//!    **nenhum INSERT acontece** e nenhuma linha nasce em `event_position`.
//!
//! O `tenant_id` do envelope nunca amplia tenancy: é conferido contra o tenant
//! autenticado do grant. Corpo com campos desconhecidos é recusado por
//! `deny_unknown_fields`, como no resto da API.
//!
//! O ACK desta fatia é sempre `persisted_local`: a escrita é local e a resposta
//! nunca alega replicação, materialização ou entrega.

use godmesh_federation::event::{SignedEvent, SignedEventError};
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

// O corpo aceito por `POST /v1/events` é `godmesh_federation::event::SignedEvent`:
// **não existe payload sem assinatura nesta rota, nem por flag**. Eventos do
// nó local (smoke/admin, `GODMESH_NODE_KEY`) entram pelo mesmo caminho: o
// processo assina com a própria chave e o nó local precisa estar registrado
// como par `authorized` com escopo concedido — zero caminho privilegiado
// implícito, zero modo de transição.

/// Janela de frescor aceita no `issued_at` do envelope.
///
/// A assinatura prova autoria, não momento: sem janela, um envelope roubado
/// vale para sempre e vira replay de histórico. Mesma regra e mesmo default do
/// manifesto de ingresso (5 minutos), mas **configuração própria** — afrouxar
/// ingresso não afrouxa ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventWindow {
    pub max_age_seconds: i64,
}

/// Nome do ambiente que ajusta a janela de frescor do ingest (segundos).
pub const EVENT_MAX_AGE_ENV: &str = "GODMESH_EVENT_MAX_AGE_SECONDS";

impl Default for EventWindow {
    fn default() -> Self {
        Self {
            max_age_seconds: godmesh_federation::manifest::DEFAULT_MAX_AGE_SECONDS,
        }
    }
}

impl EventWindow {
    /// Janela explícita; valores não positivos caem no padrão (janela zero
    /// recusaria todo evento e não é configuração útil).
    #[must_use]
    pub fn new(max_age_seconds: i64) -> Self {
        if max_age_seconds > 0 {
            Self { max_age_seconds }
        } else {
            Self::default()
        }
    }

    /// Janela lida de [`EVENT_MAX_AGE_ENV`]; ausente/ilegível fica o padrão.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var(EVENT_MAX_AGE_ENV) {
            Ok(raw) => match raw.trim().parse::<i64>() {
                Ok(seconds) => Self::new(seconds),
                Err(_) => Self::default(),
            },
            Err(_) => Self::default(),
        }
    }
}

/// Resposta do recebimento. `ack_class` é a única alegação permitida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EventPage {
    pub events: Vec<EventRecord>,
    pub next_cursor: Option<String>,
}

/// Verifica o envelope assinado **antes de qualquer efeito de escrita** e
/// persiste a posição, ou reconhece a repetição da mesma posição.
///
/// Ordem das verificações, deliberada — cada recusa devolve 422 com código
/// próprio, **sem INSERT e sem linha nova em `event_position`**:
///
/// 1. sujeito verificado e predicado do grant;
/// 2. formato do envelope e **origem = assinante** (nó comprometido não pode
///    reivindicar outro nó como origem da posição);
/// 3. o nó tem chave registrada em `federation_peer` (sem registro a recusa é
///    `event_node_unauthorized` — não existe "sem assinatura" nesta rota);
/// 4. assinatura válida sobre os bytes canônicos, com `verify_strict` contra a
///    chave do registro, reconfirmando a ligação `node_id` ↔ chave;
/// 5. `trust_state = authorized` (pendente/revogado/desconhecido nega);
/// 6. tenant assinado = tenant autenticado do grant, **e** escopo
///    `(node_id, tenant_id, stream_id)` presente em `federation_peer_scope`
///    (ausência de linha é negativa: falha fechada);
/// 7. `issued_at` dentro da janela de frescor;
/// 8. na transação da escrita: epoch não regressivo por namespace → jurisdição
///    → política de retenção → repetição → prefixo contíguo. A política vem
///    antes da repetição para que uma classe removida falhe fechado em replay.
///
/// O `now` é injetado (o caso de uso não lê o relógio sozinho) para que a
/// janela de frescor seja testável sem esperar tempo de parede.
///
/// # Errors
/// `InvalidCredential` com sujeito incompleto; `InvalidRequest` para posição ou
/// envelope malformado (campo vazio, sequência zero, fora do tipo do banco);
/// `GrantDenied` quando o predicado não devolve `Allow`; `EventSignatureInvalid`,
/// `EventNodeUnauthorized`, `EventScopeDenied`, `EventNotFresh` e
/// `EventEpochStale` para as negações desta fatia; `RetentionScopeUnresolved`
/// sem jurisdição declarada; `RetentionPolicyMissing` sem classe/cópia primária;
/// `SequenceGap` para salto no prefixo contíguo; `Storage` para falha de banco.
pub async fn create_event(
    pool: &PgPool,
    grant: &VerifiedGrant,
    event: SignedEvent,
    window: &EventWindow,
    now: i64,
) -> Result<EventOutcome, ApiError> {
    grant.validate().map_err(ApiError::InvalidCredential)?;

    match grant.predicate.evaluate() {
        godmesh_identity::grant::GrantDecision::Allow => {}
        denied => return Err(ApiError::GrantDenied(denied)),
    }

    event
        .validate_fields()
        .map_err(|e| ApiError::InvalidRequest(e.to_string()))?;
    event.verify_origin_is_signer().map_err(|e| {
        ApiError::EventScopeDenied(format!(
            "um nó não pode declarar outro como origem da posição: {e}"
        ))
    })?;

    // A posição entra no domínio antes de qualquer efeito: origem/stream vazios
    // e sequência zero são recusados pela própria `EventPosition`.
    let position = EventPosition::new(
        &event.position.origin_node,
        &event.tenant_id,
        &event.position.stream_id,
        event.position.partition,
        event.position.authority_epoch,
        event.position.seq,
    )
    .map_err(invalid_position)?;

    let partition = i32::try_from(position.partition())
        .map_err(|_| ApiError::InvalidRequest("partition fora do intervalo".to_owned()))?;
    let authority_epoch = i64::try_from(position.authority_epoch())
        .map_err(|_| ApiError::InvalidRequest("authority_epoch fora do intervalo".to_owned()))?;
    let seq = i64::try_from(position.seq())
        .map_err(|_| ApiError::InvalidRequest("seq fora do intervalo".to_owned()))?;

    let tenant_id = &grant.subject.tenant_id;
    let mut tx = begin_event_write(
        pool,
        &event.position.origin_node,
        tenant_id,
        &event.position.stream_id,
        partition,
    )
    .await?;

    // A chave que verifica a assinatura vem **só** do registro de pares aprovado
    // por ingresso (migração 0008). O envelope não carrega `public_key`: trazê-
    // la no corpo abriria um canal de troca de identidade que a assinatura não
    // fecha — quem apresenta a chave decidiria quem é.
    let peer: Option<(String, String)> = sqlx::query_as(
        "SELECT public_key, trust_state FROM federation_peer WHERE node_id = $1 FOR SHARE",
    )
    .bind(&event.node_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;
    let Some((public_key, trust_state)) = peer else {
        return Err(ApiError::EventNodeUnauthorized(format!(
            "nó {} não tem chave registrada na federação",
            event.node_id
        )));
    };

    match event.verify_signature_against(&public_key) {
        Ok(()) => {}
        Err(error) => return Err(event_signature_error(error)),
    }

    if trust_state != "authorized" {
        return Err(ApiError::EventNodeUnauthorized(format!(
            "nó {} está em estado {trust_state}; só nó autorizado escreve no log durável",
            event.node_id
        )));
    }

    // Tenancy: o tenant assinado no envelope é conferido contra o grant.
    // Assinatura de nó autorizado não amplia tenancy, e grant não escolhe
    // origem. Qualquer um dos dois mentindo é negativa de escopo.
    if &event.tenant_id != tenant_id {
        return Err(ApiError::EventScopeDenied(
            "tenant_id assinado no envelope não é o tenant autenticado pelo grant".to_owned(),
        ));
    }

    let scoped: Option<String> = sqlx::query_scalar(
        "SELECT stream_id FROM federation_peer_scope \
          WHERE node_id = $1 AND tenant_id = $2 AND stream_id = $3 FOR SHARE",
    )
    .bind(&event.node_id)
    .bind(tenant_id)
    .bind(&event.position.stream_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;
    if scoped.is_none() {
        return Err(ApiError::EventScopeDenied(format!(
            "nó {} não tem escopo para (tenant {}, stream {})",
            event.node_id, tenant_id, event.position.stream_id
        )));
    }

    if let Err(e) = event.verify_fresh(now, window.max_age_seconds) {
        return Err(ApiError::EventNotFresh(match e {
            SignedEventError::Expired {
                issued_at,
                now,
                max_age_seconds,
            } => {
                format!("issued_at {issued_at} fora da janela de {max_age_seconds}s (agora {now})")
            }
            SignedEventError::NotYetValid { issued_at, now } => {
                format!("issued_at {issued_at} no futuro além da janela (agora {now})")
            }
            other => other.to_string(),
        }));
    }

    // Epoch não regressivo por namespace `(origem, tenant, stream, partição)` —
    // a mesma dimensão do prefixo contíguo. Depois que a stream reconheceu a
    // autoridade E, um envelope assinado sob autoridade E' < E não reabre
    // histórico (fencing de ADR-061). Epoch maior é aceito: a autoridade nova
    // manda, e o prefixo dela começa do zero.
    // O agregado devolve **uma linha** mesmo sem histórico, e o valor dessa
    // linha é NULL: `fetch_optional` decodificaria NULL como i64 e o erro
    // pareceria falha de banco. Tipagem explícita em `Option<i64>` + `fetch_one`.
    let epoch_max: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT max(authority_epoch) FROM event_position \
          WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 AND partition = $4",
    )
    .bind(&event.position.origin_node)
    .bind(tenant_id)
    .bind(&event.position.stream_id)
    .bind(partition)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;
    if let Some(current) = epoch_max {
        let current_u64 = u64::try_from(current.max(0)).unwrap_or(u64::MAX);
        if position.authority_epoch() < current_u64 {
            tx.rollback().await.map_err(ApiError::Storage)?;
            return Err(ApiError::EventEpochStale {
                current: current_u64,
                got: position.authority_epoch(),
            });
        }
    }

    let jurisdiction = resolve_jurisdiction(&mut tx, tenant_id).await?;


    // Política efetiva da jurisdição do tenant, sob o mesmo lock que a escrita.
    let class = resolve_retention_class(&mut tx, &event.retention_class, &jurisdiction).await?;

    // Repetição da mesma tupla: idempotente, sem segundo INSERT. A classe da
    // resposta é a que já está durável, não a declarada no replay.
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT retention_class FROM event_position \
          WHERE origin_node = $1 AND tenant_id = $2 AND stream_id = $3 \
            AND partition = $4 AND authority_epoch = $5 AND seq = $6",
    )
    .bind(&event.position.origin_node)
    .bind(tenant_id)
    .bind(&event.position.stream_id)
    .bind(partition)
    .bind(authority_epoch)
    .bind(seq)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;

    if let Some(stored_class) = stored {
        let response = ack_for(&event, &stored_class, position.seq());
        tx.rollback().await.map_err(ApiError::Storage)?;
        return Ok(EventOutcome {
            response,
            created: false,
        });
    }

    let prefix = contiguous_prefix(
        &mut tx,
        &event.position.origin_node,
        tenant_id,
        &event.position.stream_id,
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
                response: ack_for(&event, &class, position.seq()),
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

    // A escrita normal já está serializada. A restrição única continua
    // protegendo a posição de uma escrita externa ao fluxo normal.
    let inserted = sqlx::query(
        "INSERT INTO event_position \
             (origin_node, tenant_id, stream_id, partition, authority_epoch, seq, \
              retention_class, jurisdiction) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (origin_node, tenant_id, stream_id, partition, authority_epoch, seq) \
         DO NOTHING",
    )
    .bind(&event.position.origin_node)
    .bind(tenant_id)
    .bind(&event.position.stream_id)
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
            response: ack_for(&event, &class, position.seq()),
            created: false,
        });
    }

    tx.commit().await.map_err(ApiError::Storage)?;
    Ok(EventOutcome {
        response: ack_for(&event, &class, position.seq()),
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
/// Os campos ecoam a **posição assinada**, não o corpo cru: um replay com
/// `issued_at` diferente recebe a mesma posição porque é a mesma posição.
fn ack_for(event: &SignedEvent, retention_class: &str, seq: u64) -> EventAck {
    EventAck {
        origin_node: event.position.origin_node.clone(),
        stream_id: event.position.stream_id.clone(),
        partition: event.position.partition,
        authority_epoch: event.position.authority_epoch,
        seq,
        retention_class: retention_class.to_owned(),
        ack_class: AckClass::PersistedLocal.as_str().to_owned(),
    }
}

/// Toda negação de autenticidade do envelope é recusa nomeada: a assinatura é
/// a única forma de provar autoria, e "assinatura inválida" nunca vira erro
/// genérico. Chave fora do formato no registro também nega (falha fechada).
fn event_signature_error(error: SignedEventError) -> ApiError {
    ApiError::EventSignatureInvalid(error.to_string())
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

/// Transação comum ao ingest e ao Inbox, inclusive para namespaces vazios.
/// A chave exclui o epoch para ordenar também transições de autoridade.
pub(crate) async fn begin_event_write(
    pool: &PgPool,
    origin_node: &str,
    tenant_id: &str,
    stream_id: &str,
    partition: i32,
) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, ApiError> {
    // A primeira leitura após esperar pelo lock precisa de um snapshot novo.
    let mut tx = pool
        .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
        .await
        .map_err(ApiError::Storage)?;
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended( \
            jsonb_build_array('godmesh.event_position'::text, $1::text, $2::text, \
                              $3::text, $4::int)::text, 0))",
    )
    .bind(origin_node)
    .bind(tenant_id)
    .bind(stream_id)
    .bind(partition)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;
    Ok(tx)
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
