//! Caso de uso do incidente, independente do transporte.
//!
//! Separado do HTTP de propósito: o contrato é testável sem subir servidor.
//!
//! Fronteira de confiança desta fatia: o corpo do pedido traz **só** o
//! identificador do incidente. Tenancy (`tenant_id`), autoria (`principal`) e
//! todos os campos do `GrantPredicate` vêm do grant verificado — nunca do corpo.

use godmesh_emergency::incident::{HumanProof, Incident, IncidentError, IncidentState};
use godmesh_identity::grant::GrantDecision;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::auth::{CredentialError, VerifiedGrant};

/// Corpo do pedido de incidente.
///
/// `deny_unknown_fields` é deliberado: um corpo que tente declarar `tenant_id`,
/// `principal` ou qualquer cláusula do grant é recusado antes do efeito, em vez
/// de ser aceito por conveniência. Não há campo de tenancy a divergir porque não
/// existe campo de tenancy no corpo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncidentPayload {
    pub incident_id: String,
}

/// Resposta do endpoint. `ack_class` é a única alegação permitida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentResponse {
    pub incident_id: String,
    pub state: String,
    pub ack_class: String,
    pub ui_label: String,
}

/// Resultado do caso de uso.
///
/// `created == false` significa replay idempotente do mesmo tenant: nada novo foi
/// gravado e a resposta repete o estado já persistido. O transporte traduz isso em
/// 201 (criado) ou 200 (replay), nunca em erro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOutcome {
    pub response: IncidentResponse,
    pub created: bool,
}

/// Erros de API.
#[derive(Debug)]
pub enum ApiError {
    /// Nenhuma credencial apresentada (ou desconhecida): nada foi lido nem gravado.
    Unauthenticated,
    /// Credencial reconhecida, mas sem sujeito utilizável.
    InvalidCredential(CredentialError),
    /// Pedido recusado na fronteira (corpo inválido ou tentativa de declarar tenancy).
    InvalidRequest(String),
    GrantDenied(GrantDecision),
    /// Classe sem política efetiva para a jurisdição do tenant (ADR-062).
    RetentionPolicyMissing,
    /// Salto em relação ao prefixo contíguo da stream (ADR-057): nenhuma lacuna
    /// é saltada; o chamador precisa enviar a posição esperada.
    SequenceGap { expected: u64, got: u64 },
    /// Cursor ilegível, de versão desconhecida, sem partição ou de outro
    /// principal. Distinto de cursor ausente, que começa do início.
    CursorInvalid(String),
    /// Cursor de outra partição/epoch/tenant/filtro: recusado, nunca traduzido.
    CursorIncomparable(String),
    /// Tenant sem jurisdição declarada: não há como resolver a política (ADR-066).
    RetentionScopeUnresolved,
    /// O `incident_id` já pertence a outro tenant.
    IncidentIdConflict,
    /// Incidente inexistente **ou** de outro tenant: a resposta é a mesma (404)
    /// para não confirmar a existência do identificador em outro tenant.
    IncidentNotFound,
    /// Transição fora de ordem para o estado atual; o corpo carrega o estado.
    IllegalTransition {
        /// Estado persistido no momento da recusa.
        current: String,
    },
    /// `acknowledged_human` exige prova de pessoa (`responder`).
    MissingHumanProof,
    /// O contrato do incidente recusou a transição; nunca deveria sair do servidor.
    Invariant(IncidentError),
    Storage(sqlx::Error),
}

/// Nome persistido do estado: espelha o CHECK de `sos_incident.state` em
/// `migrations/0001_core.sql`. É adaptador de armazenamento, não regra de domínio.
pub(crate) const fn state_name(state: IncidentState) -> &'static str {
    match state {
        IncidentState::TriggeredLocally => "triggered_locally",
        IncidentState::PersistedLocal => "persisted_local",
        IncidentState::ReplicatedToAuthority => "replicated_to_authority",
        IncidentState::AcknowledgedTechnical => "acknowledged_technical",
        IncidentState::AcknowledgedHuman => "acknowledged_human",
        IncidentState::OutcomeUnknown => "outcome_unknown",
        IncidentState::NotPersisted => "not_persisted",
        IncidentState::Closed => "closed",
    }
}

/// Inverso de [`state_name`]: lê o estado persistido de volta para o domínio.
pub(crate) const fn parse_state(name: &str) -> Option<IncidentState> {
    match name.as_bytes() {
        b"triggered_locally" => Some(IncidentState::TriggeredLocally),
        b"persisted_local" => Some(IncidentState::PersistedLocal),
        b"replicated_to_authority" => Some(IncidentState::ReplicatedToAuthority),
        b"acknowledged_technical" => Some(IncidentState::AcknowledgedTechnical),
        b"acknowledged_human" => Some(IncidentState::AcknowledgedHuman),
        b"outcome_unknown" => Some(IncidentState::OutcomeUnknown),
        b"not_persisted" => Some(IncidentState::NotPersisted),
        b"closed" => Some(IncidentState::Closed),
        _ => None,
    }
}

/// Nome da constraint que protege o `incident_id` global (migração 0002).
const INCIDENT_ID_UNIQUE: &str = "sos_incident_id_key";

/// Cria o incidente: exige grant verificado, resolve a política da jurisdição do
/// tenant autenticado dentro da mesma transação da escrita e grava a trilha de ACK.
///
/// Duas autoridades compõem esta fatia: o **grant** decide quem fala e com quais
/// cláusulas (`tenant_id`/`principal`/predicado); o **tenant** decide em qual
/// jurisdição a persistência é permitida (`tenant_region`). Nenhuma das duas lê
/// tenancy do corpo do pedido.
///
/// A política é lida com lock compartilhado na mesma transação do `INSERT`; a FK
/// `sos_incident_retention_class_fkey` fecha o caso de a política desaparecer
/// depois da leitura: o incidente não fica órfão. O replay é idempotente por
/// `(tenant_id, id)`, e colisão de `id` entre tenants é conflito, não segundo
/// incidente.
///
/// # Errors
/// `InvalidCredential` quando o sujeito verificado está incompleto;
/// `InvalidRequest` quando o corpo não identifica o incidente; `GrantDenied`
/// quando o predicado não devolve `Allow`; `RetentionScopeUnresolved` sem
/// jurisdição declarada para o tenant; `RetentionPolicyMissing` sem política e
/// cópia primária para a jurisdição; `IncidentIdConflict` quando o `incident_id`
/// pertence a outro tenant; `Invariant` quando o contrato recusa a transição;
/// `Storage` para falha de banco.
pub async fn create_incident(
    pool: &PgPool,
    grant: &VerifiedGrant,
    payload: IncidentPayload,
) -> Result<CreateOutcome, ApiError> {
    // Tenancy vem do sujeito autenticado; sujeito incompleto não passa daqui.
    grant.validate().map_err(ApiError::InvalidCredential)?;
    if payload.incident_id.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "incident_id vazio não identifica incidente".to_owned(),
        ));
    }

    // Exaustividade: `Allow` é o único caminho de aceitação; qualquer outra
    // variante (inclusive uma futura) cai no braço padrão e nega.
    match grant.predicate.evaluate() {
        GrantDecision::Allow => {}
        denied => return Err(ApiError::GrantDenied(denied)),
    }

    // Autorização e tenancy saem do grant verificado, nunca do corpo do pedido.
    let tenant_id = &grant.subject.tenant_id;
    let principal = &grant.subject.principal;

    // A classe de ACK vem do domínio, não de string local: `persist` é a transição
    // que autoriza alegar `persisted_local` (ADR-058).
    // O instante do ACK é real (relógio do host) e nunca zero: `sos_ack.at_unix`
    // é `NOT NULL` e um ACK sem instante não é evidência.
    let at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ApiError::Invariant(IncidentError::MissingTimestamp))?
        .as_secs();
    let mut incident = Incident::triggered(&payload.incident_id, tenant_id, principal);
    incident
        .persist(&format!("inbox:{tenant_id}"), incident.retention_class(), at_unix)
        .map_err(ApiError::Invariant)?;

    let mut tx = pool.begin().await.map_err(ApiError::Storage)?;

    // Jurisdição do tenant autenticado vem do banco, nunca do pedido (ADR-066).
    let jurisdiction: Option<String> =
        sqlx::query_scalar("SELECT jurisdiction FROM tenant_region WHERE tenant_id = $1 FOR SHARE")
            .bind(tenant_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(ApiError::Storage)?;
    let Some(jurisdiction) = jurisdiction else {
        return Err(ApiError::RetentionScopeUnresolved);
    };

    // Política efetiva para a jurisdição do tenant: classe **e** cópia primária.
    // `FOR SHARE OF` segura as duas linhas até o commit, então a checagem e a
    // escrita acontecem sob a mesma visão da política.
    let policy: Option<(String, String)> = sqlx::query_as(
        "SELECT rc.class, rc.jurisdiction \
           FROM retention_class rc \
           JOIN retention_copy cp \
             ON cp.class = rc.class \
            AND cp.jurisdiction = rc.jurisdiction \
            AND cp.copy_scope = 'primary' \
          WHERE rc.class = $1 \
            AND rc.jurisdiction = $2 \
          FOR SHARE OF rc, cp",
    )
    .bind(incident.retention_class())
    .bind(&jurisdiction)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ApiError::Storage)?;
    let Some((policy_class, policy_jurisdiction)) = policy else {
        return Err(ApiError::RetentionPolicyMissing);
    };

    let inserted = sqlx::query(
        "INSERT INTO sos_incident \
             (id, tenant_id, principal, state, retention_class, retention_jurisdiction) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (tenant_id, id) DO NOTHING",
    )
    .bind(&payload.incident_id)
    .bind(tenant_id)
    .bind(principal)
    .bind(state_name(incident.state()))
    .bind(&policy_class)
    .bind(&policy_jurisdiction)
    .execute(&mut *tx)
    .await;

    let created = match inserted {
        Ok(result) => result.rows_affected() == 1,
        Err(e) if is_incident_id_conflict(&e) => return Err(ApiError::IncidentIdConflict),
        Err(e) => return Err(ApiError::Storage(e)),
    };

    let outcome = if created {
        for ack in incident.acks() {
            sqlx::query(
                "INSERT INTO sos_ack (incident_id, ack_class, source, at_unix) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(&payload.incident_id)
            .bind(ack.class)
            .bind(&ack.source)
            .bind(i64::try_from(ack.at_unix).unwrap_or(i64::MAX))
            .execute(&mut *tx)
            .await
            .map_err(ApiError::Storage)?;
        }
        CreateOutcome {
            response: response_for(
                &payload.incident_id,
                tenant_id,
                principal,
                incident.state(),
            )?,
            created: true,
        }
    } else {
        // Replay do mesmo tenant: o incidente já existe, nada é reescrito
        // (a trilha de ACK é append-only, ADR-058). O estado é o persistido.
        let stored: String = sqlx::query_scalar(
            "SELECT state FROM sos_incident WHERE tenant_id = $1 AND id = $2",
        )
        .bind(tenant_id)
        .bind(&payload.incident_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(ApiError::Storage)?;
        let Some(state) = parse_state(&stored) else {
            return Err(ApiError::Storage(sqlx::Error::Protocol(format!(
                "estado persistido fora do conjunto fechado: {stored}"
            ))));
        };
        CreateOutcome {
            response: response_for(&payload.incident_id, tenant_id, principal, state)?,
            created: false,
        }
    };

    tx.commit().await.map_err(ApiError::Storage)?;
    Ok(outcome)
}

/// `true` quando o erro é violação da unicidade global de `incident_id`, ou seja,
/// o identificador já pertence a outro tenant.
fn is_incident_id_conflict(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => db.constraint() == Some(INCIDENT_ID_UNIQUE),
        _ => false,
    }
}

/// Monta a resposta a partir do estado que o banco guarda.
///
/// O rótulo de UI nunca é gravado nem duplicado aqui: é derivado do domínio. Como
/// o contrato de `godmesh-emergency` não expõe construtor a partir de um estado
/// qualquer, a reidratação percorre as transições públicas até o estado alvo. As
/// origens sintéticas (`replay`) servem só para obter o rótulo e nunca são
/// persistidas.
fn response_for(
    incident_id: &str,
    tenant: &str,
    principal: &str,
    state: IncidentState,
) -> Result<IncidentResponse, ApiError> {
    let incident = rehydrate(incident_id, tenant, principal, state)?;
    Ok(IncidentResponse {
        incident_id: incident_id.to_owned(),
        state: state_name(incident.state()).to_owned(),
        ack_class: incident.ack_class().to_owned(),
        ui_label: incident.ui_label().to_owned(),
    })
}

/// Reconstrói o incidente no estado persistido, sem gravar nada.
pub(crate) fn rehydrate(
    incident_id: &str,
    tenant: &str,
    principal: &str,
    state: IncidentState,
) -> Result<Incident, ApiError> {
    let mut incident = Incident::triggered(incident_id, tenant, principal);
    let inbox = format!("inbox:{tenant}");
    let retention = incident.retention_class();

    // `persist` é o primeiro degrau de todos os estados alcançáveis deste endpoint.
    let persisted = matches!(
        state,
        IncidentState::PersistedLocal
            | IncidentState::ReplicatedToAuthority
            | IncidentState::AcknowledgedTechnical
            | IncidentState::AcknowledgedHuman
            | IncidentState::Closed
            | IncidentState::OutcomeUnknown
    );
    if persisted {
        // Instante sintético (`1`, nunca `0`): a reidratação só calcula rótulo e
        // não é persistida. `at_unix == 0` é recusado pelo contrato do incidente
        // porque ACK sem instante não é evidência (ADR-058).
        const REPLAY_SYNTHETIC_AT_UNIX: u64 = 1;
        incident
            .persist(&inbox, retention, REPLAY_SYNTHETIC_AT_UNIX)
            .map_err(ApiError::Invariant)?;
    }

    match state {
        IncidentState::TriggeredLocally | IncidentState::PersistedLocal => {}
        IncidentState::NotPersisted => incident
            .fail_persistence("replay")
            .map_err(ApiError::Invariant)?,
        IncidentState::OutcomeUnknown => incident
            .mark_outcome_unknown("replay")
            .map_err(ApiError::Invariant)?,
        IncidentState::ReplicatedToAuthority
        | IncidentState::AcknowledgedTechnical
        | IncidentState::AcknowledgedHuman
        | IncidentState::Closed => {
            incident
                .mark_replicated("replay", 0)
                .map_err(ApiError::Invariant)?;
            if matches!(
                state,
                IncidentState::AcknowledgedTechnical
                    | IncidentState::AcknowledgedHuman
                    | IncidentState::Closed
            ) {
                incident
                    .mark_technical_ack("replay", 0)
                    .map_err(ApiError::Invariant)?;
            }
            if matches!(state, IncidentState::AcknowledgedHuman | IncidentState::Closed) {
                incident
                    .mark_acknowledged_human(HumanProof::person("replay", 0))
                    .map_err(ApiError::Invariant)?;
            }
            if state == IncidentState::Closed {
                incident.close("replay").map_err(ApiError::Invariant)?;
            }
        }
    }
    Ok(incident)
}