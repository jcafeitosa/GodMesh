//! Listagem paginada de incidentes do tenant autenticado.
//!
//! Regras desta fatia, todas vindas de contrato já existente:
//!  * **tenancy** — `tenant_id` sai do `VerifiedGrant`; a query sempre filtra por
//!    ele, então incidente de outro tenant não aparece nem por engano de cursor;
//!  * **`limit` obrigatório, com teto** — ausente é 422 (`limit` sem default é
//!    parâmetro inventado), `0` e `> 100` são 422. O teto é **recusado**, nunca
//!    truncado em silêncio: uma página que o servidor corta sozinho faria o
//!    cliente perder linhas sem saber;
//!  * **cursor opaco e vinculado** — o token é o de
//!    [`godmesh_protocol::cursor`] (ADR-057), amarrado a
//!    `(tenant, coleção, partição, epoch, principal, filtros)`. Cursor de outro
//!    tenant ou de outro filtro é `400 cursor_incomparable`; ilegível é
//!    `400 cursor_invalid`; ausente começa do início;
//!  * **ordem estável** — `list_seq DESC` (ordem de inserção, atribuída pelo
//!    banco), publicada no contrato: a página seguinte é `list_seq < posição`,
//!    então inserção concorrente não repete nem pula linha (paginação por
//!    `OFFSET` repetiria).
//!
//! Nenhuma resposta promete atendimento, entrega ou resolução: `ack_class` e
//! `ui_label` derivam do estado persistido, como na leitura de um incidente.

use godmesh_identity::grant::GrantDecision;
use godmesh_protocol::cursor::{Cursor, CursorScope};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::api::{ApiError, parse_state, rehydrate, state_name};
use crate::auth::VerifiedGrant;
use crate::events::{cursor_error, cursor_token_error};

/// Teto de página desta fatia. Limite de implementação, não SLO nem promessa de
/// capacidade.
pub const MAX_INCIDENTS_LIMIT: u32 = 100;

/// Coleção no escopo do cursor. Fixa: só existe uma listagem de incidentes.
const CURSOR_STREAM: &str = "incidents";

/// Partição do escopo de cursor. A listagem não é particionada, mas a ADR-057
/// exige partição explícita e recusa a sentinela de ausência — `0` declara "sem
/// partição de stream", em vez de deixar o campo em branco.
const CURSOR_PARTITION: u32 = 0;

/// Perfil de schema gravado no token. Um cursor de outro perfil não é
/// reinterpretado.
pub const CURSOR_PROFILE: &str = "gmp.incidents.v1";

/// Escopo de retenção gravado no token: a classe dos registros listados. Um
/// cursor emitido para outra classe de dado não serve a esta consulta.
const CURSOR_RETENTION: &str = "incidente_sos";

/// Filtros da listagem. `limit` é obrigatório e não tem default.
///
/// `deny_unknown_fields` recusa `tenant_id`/`principal` na query: tenancy vem do
/// grant verificado, não da URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncidentListQuery {
    pub limit: u32,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
}

/// Um incidente na listagem. Sem a trilha de ACK: a lista é índice, e a trilha é
/// leitura de um incidente (`GET /v1/incidents/{id}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IncidentSummary {
    pub incident_id: String,
    pub state: String,
    pub ack_class: String,
    pub ui_label: String,
    /// Instante de criação, segundos desde a época UNIX (fonte: `created_at`).
    pub created_at_unix: i64,
}

/// Página de incidentes. `next_cursor` nulo significa fim do intervalo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IncidentPage {
    pub incidents: Vec<IncidentSummary>,
    pub next_cursor: Option<String>,
}

/// Lista incidentes do tenant autenticado, em `list_seq DESC`.
///
/// # Errors
/// `InvalidCredential` com sujeito verificado incompleto; `GrantDenied` quando o
/// predicado do grant não devolve `Allow`; `InvalidRequest` para `limit` fora de
/// `1..=MAX_INCIDENTS_LIMIT`, `state` vazio ou fora do conjunto fechado;
/// `CursorInvalid`/`CursorIncomparable` conforme o token; `Storage` em falha de
/// banco ou estado persistido fora do conjunto fechado.
pub async fn list_incidents(
    pool: &PgPool,
    grant: &VerifiedGrant,
    query: IncidentListQuery,
) -> Result<IncidentPage, ApiError> {
    grant.validate().map_err(ApiError::InvalidCredential)?;

    match grant.predicate.evaluate() {
        GrantDecision::Allow => {}
        denied => return Err(ApiError::GrantDenied(denied)),
    }

    if query.limit == 0 || query.limit > MAX_INCIDENTS_LIMIT {
        return Err(ApiError::InvalidRequest(format!(
            "limit precisa estar entre 1 e {MAX_INCIDENTS_LIMIT}"
        )));
    }

    // `state` é validado contra o conjunto fechado do contrato antes de virar
    // filtro: estado desconhecido é pedido inválido, não lista vazia.
    let state_filter = match query.state.as_deref().map(str::trim) {
        None => None,
        Some("") => {
            return Err(ApiError::InvalidRequest(
                "state vazio não é filtro; omita o parâmetro para listar todos".to_owned(),
            ));
        }
        Some(raw) => {
            let Some(state) = parse_state(raw) else {
                return Err(ApiError::InvalidRequest(format!(
                    "state desconhecido: {raw}"
                )));
            };
            Some(state_name(state))
        }
    };

    let tenant_id = &grant.subject.tenant_id;
    let scope = CursorScope::new(
        tenant_id,
        CURSOR_STREAM,
        CURSOR_PARTITION,
        grant.authority_epoch,
    )
    .map_err(|e| ApiError::InvalidRequest(format!("escopo de cursor inválido: {e:?}")))?;
    let filters = filters_for(state_filter);

    let mut cursor = Cursor::new(
        scope.clone(),
        &grant.subject.principal,
        &filters,
        CURSOR_PROFILE,
        CURSOR_RETENTION,
    )
    .map_err(|e| ApiError::InvalidRequest(format!("escopo de cursor inválido: {e:?}")))?;

    let mut after = i64::MAX;
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

    let fetch = i64::from(query.limit) + 1; // +1 só para saber se há próxima página

    let rows: Vec<(String, String, String, i64, i64)> = sqlx::query_as(
        "SELECT id, principal, state, (EXTRACT(EPOCH FROM created_at))::bigint, list_seq \
           FROM sos_incident \
          WHERE tenant_id = $1 \
            AND ($2::text IS NULL OR state = $2) \
            AND list_seq < $3 \
          ORDER BY list_seq DESC \
          LIMIT $4",
    )
    .bind(tenant_id)
    .bind(state_filter)
    .bind(after)
    .bind(fetch)
    .fetch_all(pool)
    .await
    .map_err(ApiError::Storage)?;

    let has_more = rows.len() > query.limit as usize;
    let mut incidents = Vec::with_capacity(rows.len());
    for row in rows.iter().take(query.limit as usize) {
        incidents.push(summary_from_row(row, tenant_id)?);
    }

    let next_cursor = if has_more {
        let last = incidents.len();
        // A posição é o `list_seq` da última linha entregue. Guardado na 5ª
        // coluna da tupla, nunca recalculado a partir de `created_at`.
        let last_seq = rows
            .get(last.saturating_sub(1))
            .map(|row| row.4)
            .ok_or_else(|| storage_invalid("página vazia com next_cursor"))?;
        let last_seq = u64::try_from(last_seq)
            .map_err(|_| storage_invalid("list_seq persistida fora do intervalo"))?;
        Some(cursor.encode(last_seq))
    } else {
        None
    };

    Ok(IncidentPage {
        incidents,
        next_cursor,
    })
}

/// Converte a linha em resposta. O estado persistido fora do conjunto fechado é
/// invariante de armazenamento, nunca corrigido por inferência.
fn summary_from_row(
    row: &(String, String, String, i64, i64),
    tenant_id: &str,
) -> Result<IncidentSummary, ApiError> {
    let (incident_id, stored_principal, stored_state, created_at_unix, _) = row;
    let Some(state) = parse_state(stored_state) else {
        return Err(ApiError::Storage(sqlx::Error::Protocol(format!(
            "estado persistido fora do conjunto fechado: {stored_state}"
        ))));
    };
    // O autor do incidente é o persistido, não quem chama a listagem: senão o
    // rótulo sairia do chamador e não do dado.
    let incident = rehydrate(incident_id, tenant_id, stored_principal, state)?;
    Ok(IncidentSummary {
        incident_id: incident_id.clone(),
        state: state_name(incident.state()).to_owned(),
        ack_class: incident.ack_class().to_owned(),
        ui_label: incident.ui_label().to_owned(),
        created_at_unix: *created_at_unix,
    })
}

/// Filtro em forma canônica: entra no token e precisa ser idêntico na retomada,
/// senão o cursor é de outra consulta.
fn filters_for(state: Option<&str>) -> String {
    match state {
        Some(state) => format!("state={state}"),
        None => "state=*".to_owned(),
    }
}

fn storage_invalid(detail: &str) -> ApiError {
    ApiError::Storage(sqlx::Error::Protocol(detail.to_owned()))
}
