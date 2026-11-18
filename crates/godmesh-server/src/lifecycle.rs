//! Ciclo de vida do incidente no servidor: leitura do estado e transições.
//!
//! Fronteiras desta fatia, todas não negociáveis:
//!  * **tenancy** — `tenant_id` sai do `VerifiedGrant`, nunca do caminho nem do
//!    corpo; incidente de outro tenant responde **404**, não 403, para não
//!    confirmar que o identificador existe em outro tenant;
//!  * **estado é a única fonte do rótulo** — `ack_class` e `ui_label` derivam do
//!    estado persistido (`godmesh-emergency`), nunca do pedido nem de um campo
//!    aceito do cliente (ADR-058);
//!  * **trilha append-only** — cada transição grava um `sos_ack` imutável com
//!    `at_unix` real (> 0); nada é atualizado nem apagado (ADR-058);
//!  * **concorrência** — a transição é um `UPDATE … WHERE state = $esperado
//!    RETURNING`; 0 linhas é conflito (409) e não grava ACK, então duas chamadas
//!    simultâneas nunca duplicam evidência.
//!
//! Nenhuma resposta desta fatia promete entrega, atendimento ou sucesso de
//! resgate: `ack_class` é o que o estado prova, e nada mais.

use godmesh_identity::grant::GrantDecision;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::api::{parse_state, rehydrate, state_name, ApiError};
use crate::auth::VerifiedGrant;

/// Um registro da trilha de ACK, como o banco guarda.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckView {
    pub ack_class: String,
    pub source: String,
    pub at_unix: i64,
}

/// Estado observável do incidente.
///
/// `state`, `ack_class` e `ui_label` vêm todos do estado persistido; `acks` é a
/// trilha na ordem em que foi gravada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentView {
    pub incident_id: String,
    pub state: String,
    pub ack_class: String,
    pub ui_label: String,
    pub acks: Vec<AckView>,
}

/// Corpo do reconhecimento humano.
///
/// `responder` é a prova de que uma **pessoa** reconheceu. Sem ele, a transição
/// para `acknowledged_human` não acontece (`422 missing_human_proof`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcknowledgePayload {
    #[serde(default)]
    pub responder: Option<String>,
}

/// Transições expostas por esta fatia.
///
/// `expected` é o único estado de origem aceito e `next` o único destino. O par é
/// fixo: nenhum estado do pedido escolhe destino. `ack_class` é a classe de ACK
/// **gravada na trilha** por aquela transição (`None` quando a transição não cria
/// evidência nova — o encerramento mantém o ACK humano já existente).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// `persisted_local → replicated_to_authority`.
    Replicate,
    /// `replicated_to_authority → acknowledged_technical`.
    AcknowledgeTechnical,
    /// `acknowledged_technical → acknowledged_human` (exige pessoa).
    AcknowledgeHuman,
    /// `acknowledged_human → closed`.
    Close,
}

impl Transition {
    /// Estado de origem obrigatório.
    #[must_use]
    pub const fn expected(self) -> &'static str {
        match self {
            Self::Replicate => "persisted_local",
            Self::AcknowledgeTechnical => "replicated_to_authority",
            Self::AcknowledgeHuman => "acknowledged_technical",
            Self::Close => "acknowledged_human",
        }
    }

    /// Estado de destino.
    #[must_use]
    pub const fn next(self) -> &'static str {
        match self {
            Self::Replicate => "replicated_to_authority",
            Self::AcknowledgeTechnical => "acknowledged_technical",
            Self::AcknowledgeHuman => "acknowledged_human",
            Self::Close => "closed",
        }
    }

    /// Classe de ACK gravada na trilha, se a transição produz evidência nova.
    #[must_use]
    pub const fn ack_class(self) -> Option<&'static str> {
        match self {
            Self::Replicate => Some("replicated_to_authority"),
            Self::AcknowledgeTechnical => Some("materialized"),
            Self::AcknowledgeHuman => Some("acknowledged_human"),
            Self::Close => None,
        }
    }
}

/// Lê o incidente do tenant autenticado.
///
/// # Errors
/// `InvalidCredential` quando o sujeito do grant está incompleto;
/// `IncidentNotFound` quando o incidente não existe **ou** pertence a outro
/// tenant (mesma resposta: existência não vaza); `Storage` em falha de banco.
pub async fn load_incident(
    pool: &PgPool,
    grant: &VerifiedGrant,
    incident_id: &str,
) -> Result<IncidentView, ApiError> {
    grant.validate().map_err(ApiError::InvalidCredential)?;
    let tenant = &grant.subject.tenant_id;

    let row: Option<(String, String)> =
        sqlx::query_as("SELECT principal, state FROM sos_incident WHERE tenant_id = $1 AND id = $2")
            .bind(tenant)
            .bind(incident_id)
            .fetch_optional(pool)
            .await
            .map_err(ApiError::Storage)?;
    let Some((principal, stored_state)) = row else {
        return Err(ApiError::IncidentNotFound);
    };

    let acks = read_acks(pool, incident_id).await?;
    view(incident_id, tenant, &principal, &stored_state, acks)
}

/// Executa uma transição legal do incidente do tenant autenticado.
///
/// A atomicidade vem do banco, não do processo: `UPDATE … WHERE tenant_id = $1
/// AND id = $2 AND state = $esperado RETURNING state`. A linha do incidente é
/// travada pelo `UPDATE`, então a segunda chamada concorrente reavalia o `WHERE`
/// contra o estado já commitado e recebe 0 linhas → `IllegalTransition` (409).
/// Como o ACK só é inserido depois do `UPDATE` bem-sucedido, na mesma transação,
/// exatamente uma chamada grava a evidência.
///
/// # Errors
/// `InvalidCredential` com sujeito incompleto; `MissingHumanProof` quando a
/// transição exige pessoa e `responder` está ausente/vazio; `IncidentNotFound`
/// quando o incidente não existe no tenant; `IllegalTransition` com o estado
/// atual no erro; `Storage` em falha de banco.
pub async fn transition(
    pool: &PgPool,
    grant: &VerifiedGrant,
    incident_id: &str,
    kind: Transition,
    responder: Option<&str>,
) -> Result<IncidentView, ApiError> {
    grant.validate().map_err(ApiError::InvalidCredential)?;
    let tenant = &grant.subject.tenant_id;

    // Prova de pessoa é checada antes de qualquer efeito: pedido sem prova não
    // pode tomar o lock do incidente nem consumir a transição.
    let responder = match kind {
        Transition::AcknowledgeHuman => {
            let Some(name) = responder.map(str::trim).filter(|n| !n.is_empty()) else {
                return Err(ApiError::MissingHumanProof);
            };
            Some(name.to_owned())
        }
        Transition::Replicate | Transition::AcknowledgeTechnical | Transition::Close => None,
    };

    // A admissão do pedido segue o mesmo predicado verificado do grant: uma
    // credencial que nega não abre transição. O estado, porém, nunca vem daqui.
    match grant.predicate.evaluate() {
        GrantDecision::Allow => {}
        denied => return Err(ApiError::GrantDenied(denied)),
    }

    let at_unix = now_unix()?;

    let mut tx = pool.begin().await.map_err(ApiError::Storage)?;

    let updated: Option<(String, String)> = sqlx::query_as(
        "UPDATE sos_incident \
            SET state = $3, updated_at = now() \
          WHERE tenant_id = $1 AND id = $2 AND state = $4 \
        RETURNING principal, state",
    )
    .bind(tenant)
    .bind(incident_id)
    .bind(kind.next())
    .bind(kind.expected())
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;

    let Some((principal_stored, new_state)) = updated else {
        // 0 linhas: ou o incidente não é deste tenant (404), ou o estado atual
        // não permite a transição (409 com o estado atual no corpo).
        return Err(classify_zero_rows(&mut tx, tenant, incident_id).await?);
    };

    if let Some(ack_class) = kind.ack_class() {
        let source = match (kind, responder.as_deref()) {
            (Transition::Replicate | Transition::AcknowledgeTechnical, _) => {
                // Emissor técnico: a autoridade de destino, não o cliente.
                "authority:godmesh-server".to_owned()
            }
            (Transition::AcknowledgeHuman, Some(name)) => name.to_owned(),
            (Transition::AcknowledgeHuman, None) => {
                // Inalcançável: barrado antes da transação.
                return Err(ApiError::MissingHumanProof);
            }
            (Transition::Close, _) => unreachable!("encerramento não grava ACK novo"),
        };
        sqlx::query(
            "INSERT INTO sos_ack (incident_id, ack_class, source, at_unix) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(incident_id)
        .bind(ack_class)
        .bind(&source)
        .bind(i64::try_from(at_unix).unwrap_or(i64::MAX))
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Storage)?;
    }

    let acks = read_acks_tx(&mut tx, incident_id).await?;
    tx.commit().await.map_err(ApiError::Storage)?;

    view(incident_id, tenant, &principal_stored, &new_state, acks)
}

/// Distingue as duas causas de 0 linhas no `UPDATE`, sem vazar existência:
/// incidente de outro tenant e incidente inexistente respondem igual (404).
async fn classify_zero_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    tenant: &str,
    incident_id: &str,
) -> Result<ApiError, ApiError> {
    let current: Option<String> =
        sqlx::query_scalar("SELECT state FROM sos_incident WHERE tenant_id = $1 AND id = $2")
            .bind(tenant)
            .bind(incident_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(ApiError::Storage)?;
    Ok(match current {
        Some(state) => ApiError::IllegalTransition { current: state },
        None => ApiError::IncidentNotFound,
    })
}

/// Instante real do ACK: relógio do host, segundos desde a época UNIX.
/// `at_unix == 0` é recusado porque ACK sem instante não é evidência (ADR-058).
fn now_unix() -> Result<u64, ApiError> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApiError::Invariant(godmesh_emergency::incident::IncidentError::MissingTimestamp))?
        .as_secs();
    if secs == 0 {
        return Err(ApiError::Invariant(
            godmesh_emergency::incident::IncidentError::MissingTimestamp,
        ));
    }
    Ok(secs)
}

async fn read_acks(pool: &PgPool, incident_id: &str) -> Result<Vec<AckView>, ApiError> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT ack_class, source, at_unix FROM sos_ack \
          WHERE incident_id = $1 \
          ORDER BY recorded_at, at_unix, ack_class",
    )
    .bind(incident_id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::Storage)?;
    Ok(rows
        .into_iter()
        .map(|(ack_class, source, at_unix)| AckView {
            ack_class,
            source,
            at_unix,
        })
        .collect())
}

async fn read_acks_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    incident_id: &str,
) -> Result<Vec<AckView>, ApiError> {
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT ack_class, source, at_unix FROM sos_ack \
          WHERE incident_id = $1 \
          ORDER BY recorded_at, at_unix, ack_class",
    )
    .bind(incident_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(ApiError::Storage)?;
    Ok(rows
        .into_iter()
        .map(|(ack_class, source, at_unix)| AckView {
            ack_class,
            source,
            at_unix,
        })
        .collect())
}

/// Monta a resposta a partir do estado persistido.
///
/// `ack_class` e `ui_label` saem do domínio reidratado, nunca de um campo do
/// pedido; estado fora do conjunto fechado é invariante de armazenamento.
fn view(
    incident_id: &str,
    tenant: &str,
    principal: &str,
    stored_state: &str,
    acks: Vec<AckView>,
) -> Result<IncidentView, ApiError> {
    let Some(state) = parse_state(stored_state) else {
        return Err(ApiError::Storage(sqlx::Error::Protocol(format!(
            "estado persistido fora do conjunto fechado: {stored_state}"
        ))));
    };
    let incident = rehydrate(incident_id, tenant, principal, state)?;
    Ok(IncidentView {
        incident_id: incident_id.to_owned(),
        state: state_name(incident.state()).to_owned(),
        ack_class: incident.ack_class().to_owned(),
        ui_label: incident.ui_label().to_owned(),
        acks,
    })
}