//! Federação: pedidos de ingresso e registro de pares.
//!
//! Caso de uso independente do HTTP, como `admin.rs`: a rota traduz, este módulo
//! decide. Regras que valem aqui (ADR-071, fatia M1):
//!
//! * um pedido de ingresso só é gravado depois de o manifesto assinar certo,
//!   ligar `node_id`↔chave pública e estar dentro da janela de frescor;
//! * o braço **público** da federação (`POST /v1/federation/join-requests`) não
//!   cria par autorizado: ele cria `pending`. Confiança só nasce de aprovação
//!   administrativa;
//! * aprovar é idempotente para um pedido já aprovado e **não** ressuscita par
//!   revogado (`revoked → authorized` é ilegal): revogado precisa voltar por
//!   `pending` e nova decisão;
//! * idempotência de ingresso é `(node_id, epoch)` — reenviar o mesmo manifesto
//!   devolve o pedido existente sem duplicar linha e sem reescrever
//!   `requested_at`.
//!
//! Nenhuma função aqui lê, grava ou registra chave privada: o manifesto só
//! carrega chave pública, e o servidor nem tem identidade de nó nesta fatia.

use godmesh_federation::SignedManifest;
use godmesh_federation::manifest::ManifestError;
use godmesh_federation::peer::{PeerRecord, TrustState};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use crate::api::ApiError;

/// Janela padrão de frescor do manifesto: 5 minutos.
pub const DEFAULT_JOIN_MAX_AGE_SECONDS: i64 = 300;

/// Variável de ambiente que ajusta a janela de frescor (segundos).
pub const JOIN_MAX_AGE_ENV: &str = "GODMESH_FEDERATION_JOIN_MAX_AGE_SECONDS";

/// Janela de aceitação de `issued_at` no ingresso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinWindow {
    /// Idade máxima aceita, em segundos (vale também para o futuro).
    pub max_age_seconds: i64,
}

impl Default for JoinWindow {
    fn default() -> Self {
        Self {
            max_age_seconds: DEFAULT_JOIN_MAX_AGE_SECONDS,
        }
    }
}

impl JoinWindow {
    /// Janela explícita. Valores não positivos caem no padrão: janela zero
    /// recusaria todo manifesto e não é uma configuração útil.
    #[must_use]
    pub fn new(max_age_seconds: i64) -> Self {
        if max_age_seconds > 0 {
            Self { max_age_seconds }
        } else {
            Self::default()
        }
    }

    /// Janela lida de `GODMESH_FEDERATION_JOIN_MAX_AGE_SECONDS`, com padrão de
    /// 5 minutos. Valor ilegível não derruba o processo: fica o padrão e o
    /// operador vê a janela efetiva na resposta do pedido? Não — não há eco de
    /// configuração. O valor efetivo aparece em `GET /v1/federation/join-requests`
    /// apenas pelo efeito (aceito/recusado).
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var(JOIN_MAX_AGE_ENV) {
            Ok(raw) => match raw.trim().parse::<i64>() {
                Ok(seconds) => Self::new(seconds),
                Err(_) => Self::default(),
            },
            Err(_) => Self::default(),
        }
    }
}

/// Pedido de ingresso como a API devolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinRequestView {
    /// Identificador público do pedido (`fjr_` + 32 hex).
    pub join_request_id: String,
    /// `node_id` do nó que pediu ingresso.
    pub node_id: String,
    /// Geração da identidade de federação.
    pub epoch: u64,
    /// `pending` | `approved` | `rejected` | `expired`.
    pub state: String,
    /// Instante do pedido (segundos Unix).
    pub requested_at: i64,
    /// Instante da aprovação, quando aprovado.
    pub approved_at: Option<i64>,
    /// Principal do grant que aprovou, quando aprovado.
    pub approved_by: Option<String>,
    /// Manifesto assinado como chegou (evidência do que foi pedido).
    pub manifest: SignedManifest,
}

/// Resultado de um pedido de ingresso: a resposta e se ele nasceu agora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinOutcome {
    /// Pedido persistido (novo ou o já existente da mesma geração).
    pub request: JoinRequestView,
    /// `true` quando esta chamada criou a linha.
    pub created: bool,
}

impl From<ManifestError> for ApiError {
    fn from(e: ManifestError) -> Self {
        match e {
            ManifestError::SignatureInvalid
            | ManifestError::InvalidSignatureEncoding(_)
            | ManifestError::InvalidPublicKey(_) => Self::InvalidManifestSignature,
            ManifestError::Expired { .. } => Self::ManifestExpired,
            ManifestError::NotYetValid { .. } => Self::ManifestNotYetValid,
            ManifestError::NodeIdMismatch => {
                Self::InvalidManifest("node_id não é o derivado da chave pública".to_owned())
            }
            other => Self::InvalidManifest(other.to_string()),
        }
    }
}

fn new_join_request_id() -> Result<String, ApiError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| ApiError::RandomnessUnavailable(e.to_string()))?;
    Ok(format!("fjr_{}", hex::encode(bytes)))
}

/// Leitura de pedido de ingresso. O manifesto vem como **texto** e é parseado
/// com `serde_json`: assim não é preciso habilitar a feature `json` do sqlx só
/// para uma coluna. SQL estático (sem interpolação) em todas as variantes.
macro_rules! join_query {
    ($tail:expr) => {
        concat!(
            "SELECT join_request_id, node_id, epoch, manifest::text AS manifest, ",
            "extract(epoch from requested_at)::bigint AS requested_at, ",
            "extract(epoch from approved_at)::bigint AS approved_at, approved_by, state ",
            "FROM federation_join_request ",
            $tail
        )
    };
}

fn join_view(row: &sqlx::postgres::PgRow) -> Result<JoinRequestView, ApiError> {
    let mut view = JoinRequestView {
        join_request_id: row.try_get("join_request_id")?,
        node_id: row.try_get("node_id")?,
        epoch: row.try_get::<i64, _>("epoch")? as u64,
        state: row.try_get("state")?,
        requested_at: row.try_get("requested_at")?,
        approved_at: row.try_get("approved_at")?,
        approved_by: row.try_get("approved_by")?,
        manifest: {
            let raw: String = row.try_get("manifest")?;
            serde_json::from_str(&raw)
                .map_err(|e| ApiError::FederationInvariant(format!("manifesto ilegível: {e}")))?
        },
    };
    // `manifest` é a fonte de `node_id`/`epoch`; divergência aqui é corrupção de
    // banco, não pedido do cliente.
    if view.manifest.node_id != view.node_id || view.manifest.epoch != view.epoch {
        return Err(ApiError::FederationInvariant(format!(
            "pedido {} divergente do manifesto (node_id/epoch)",
            view.join_request_id
        )));
    }
    view.epoch = view.manifest.epoch;
    Ok(view)
}

/// Grava (ou reaproveita) um pedido de ingresso a partir de um manifesto.
///
/// Valida assinatura, ligação `node_id`↔chave e frescor **antes** de qualquer
/// escrita. Manifesto recusado não deixa linha.
///
/// # Errors
/// [`ApiError::InvalidManifestSignature`] (401), [`ApiError::ManifestExpired`] /
/// [`ApiError::ManifestNotYetValid`] / [`ApiError::InvalidManifest`] (422) e
/// [`ApiError::Storage`] (503).
pub async fn submit_join_request(
    pool: &PgPool,
    window: JoinWindow,
    manifest: &SignedManifest,
    now: i64,
) -> Result<JoinOutcome, ApiError> {
    manifest.verify_fresh(now, window.max_age_seconds)?;

    let manifest_json = serde_json::to_value(manifest)
        .map_err(|e| ApiError::InvalidManifest(format!("manifesto não serializa: {e}")))?;
    let join_request_id = new_join_request_id()?;

    let inserted: Option<String> = sqlx::query_scalar(
        "INSERT INTO federation_join_request \
             (join_request_id, node_id, epoch, manifest, requested_at, state) \
         VALUES ($1, $2, $3, $4::jsonb, to_timestamp($5::double precision), 'pending') \
         ON CONFLICT (node_id, epoch) DO NOTHING \
         RETURNING join_request_id",
    )
    .bind(&join_request_id)
    .bind(&manifest.node_id)
    .bind(manifest.epoch as i64)
    .bind(manifest_json.to_string())
    .bind(now as f64)
    .fetch_optional(pool)
    .await?;

    let created = inserted.is_some();
    // Se o INSERT não criou nada, o pedido daquela geração já existe: devolve o
    // mesmo registro, sem reescrever `requested_at` nem a decisão já tomada.
    let row = sqlx::query(join_query!("WHERE node_id = $1 AND epoch = $2"))
        .bind(&manifest.node_id)
        .bind(manifest.epoch as i64)
        .fetch_one(pool)
        .await?;
    Ok(JoinOutcome {
        request: join_view(&row)?,
        created,
    })
}

/// Lista pedidos de ingresso, opcionalmente filtrando por estado.
///
/// # Errors
/// Estado fora do contrato é `422`; falha de banco é `503`.
pub async fn list_join_requests(
    pool: &PgPool,
    state: Option<&str>,
) -> Result<Vec<JoinRequestView>, ApiError> {
    let rows = match state {
        Some(state) => {
            if !matches!(state, "pending" | "approved" | "rejected" | "expired") {
                return Err(ApiError::InvalidRequest(format!(
                    "state desconhecido: {state}; use pending, approved, rejected ou expired"
                )));
            }
            sqlx::query(join_query!(
                "WHERE state = $1 ORDER BY requested_at DESC, join_request_id"
            ))
            .bind(state)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(join_query!("ORDER BY requested_at DESC, join_request_id"))
                .fetch_all(pool)
                .await?
        }
    };
    rows.iter().map(join_view).collect()
}

/// Aprova um pedido pendente e cria/atualiza o par como `authorized`.
///
/// Idempotente para pedido já aprovado. Pedido `rejected`/`expired` é `409`.
/// Par existente passa pela máquina de estados: `pending → authorized` e
/// `authorized → authorized` valem; `revoked → authorized` **não** vale.
///
/// # Errors
/// `404` pedido inexistente; `409` estado que não aceita aprovação; `503` banco.
pub async fn approve_join_request(
    pool: &PgPool,
    join_request_id: &str,
    approved_by: &str,
    now: i64,
) -> Result<(JoinRequestView, PeerRecord), ApiError> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query(join_query!("WHERE join_request_id = $1 FOR UPDATE"))
        .bind(join_request_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::JoinRequestNotFound)?;
    let mut request = join_view(&row)?;

    if request.state != "pending" && request.state != "approved" {
        return Err(ApiError::JoinRequestStateConflict {
            state: request.state,
        });
    }

    let existing: Option<String> =
        sqlx::query_scalar("SELECT trust_state FROM federation_peer WHERE node_id = $1 FOR UPDATE")
            .bind(&request.manifest.node_id)
            .fetch_optional(&mut *tx)
            .await?;

    let mut peer = PeerRecord::from_manifest(&request.manifest, now);
    if let Some(current_name) = existing {
        let current = TrustState::from_name(&current_name).ok_or_else(|| {
            ApiError::FederationInvariant(format!(
                "par {} com trust_state ilegível: {current_name}",
                request.manifest.node_id
            ))
        })?;
        peer.trust_state = current;
        peer.transition_to(TrustState::Authorized)
            .map_err(|e| ApiError::TrustTransition {
                current: e.current.as_str().to_owned(),
                next: e.next.as_str().to_owned(),
            })?;
    } else {
        peer.transition_to(TrustState::Authorized)
            .map_err(|e| ApiError::TrustTransition {
                current: e.current.as_str().to_owned(),
                next: e.next.as_str().to_owned(),
            })?;
    }

    sqlx::query(
        "INSERT INTO federation_peer \
             (node_id, peer_id, public_key, cluster, region, trust_state, endpoints, \
              software_version, capabilities, epoch, last_seen_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, \
                 to_timestamp($11::double precision), now()) \
         ON CONFLICT (node_id) DO UPDATE SET \
             peer_id = EXCLUDED.peer_id, \
             public_key = EXCLUDED.public_key, \
             cluster = EXCLUDED.cluster, \
             region = EXCLUDED.region, \
             trust_state = EXCLUDED.trust_state, \
             endpoints = EXCLUDED.endpoints, \
             software_version = EXCLUDED.software_version, \
             capabilities = EXCLUDED.capabilities, \
             epoch = EXCLUDED.epoch, \
             last_seen_at = EXCLUDED.last_seen_at, \
             updated_at = now()",
    )
    .bind(&peer.node_id)
    .bind(&peer.peer_id)
    .bind(&peer.public_key)
    .bind(&peer.cluster)
    .bind(&peer.region)
    .bind(peer.trust_state.as_str())
    .bind(&peer.endpoints)
    .bind(&peer.software_version)
    .bind(&peer.capabilities)
    .bind(peer.epoch as i64)
    .bind(now as f64)
    .execute(&mut *tx)
    .await?;

    // Aprovar de novo não reescreve o carimbo original: a evidência da primeira
    // decisão é o que se preserva.
    if request.state == "pending" {
        sqlx::query(
            "UPDATE federation_join_request \
                SET state = 'approved', \
                    approved_at = to_timestamp($2::double precision), \
                    approved_by = $3 \
              WHERE join_request_id = $1 AND state = 'pending'",
        )
        .bind(join_request_id)
        .bind(now as f64)
        .bind(approved_by)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let row = sqlx::query(join_query!("WHERE join_request_id = $1"))
        .bind(join_request_id)
        .fetch_one(pool)
        .await?;
    request = join_view(&row)?;
    Ok((request, peer))
}

/// Rejeita um pedido pendente. **Nenhum par é criado.**
///
/// # Errors
/// `404` pedido inexistente; `409` pedido já decidido; `503` banco.
pub async fn reject_join_request(
    pool: &PgPool,
    join_request_id: &str,
) -> Result<JoinRequestView, ApiError> {
    let row = sqlx::query(join_query!(
        "WHERE join_request_id = $1 AND state = 'pending'"
    ))
    .bind(join_request_id)
    .fetch_optional(pool)
    .await?;

    if row.is_none() {
        let state: Option<String> = sqlx::query_scalar(
            "SELECT state FROM federation_join_request WHERE join_request_id = $1",
        )
        .bind(join_request_id)
        .fetch_optional(pool)
        .await?;
        return Err(match state {
            Some(state) => ApiError::JoinRequestStateConflict { state },
            None => ApiError::JoinRequestNotFound,
        });
    }

    sqlx::query(
        "UPDATE federation_join_request SET state = 'rejected' \
          WHERE join_request_id = $1 AND state = 'pending'",
    )
    .bind(join_request_id)
    .execute(pool)
    .await?;

    let row = sqlx::query(join_query!("WHERE join_request_id = $1"))
        .bind(join_request_id)
        .fetch_one(pool)
        .await?;
    join_view(&row)
}

/// Lista pares registrados. `public_key` é dado público; não há segredo aqui.
///
/// # Errors
/// `503` banco, ou estado de confiança persistido ilegível (invariante).
pub async fn list_peers(pool: &PgPool) -> Result<Vec<PeerRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT node_id, peer_id, public_key, cluster, region, trust_state, endpoints, \
                software_version, capabilities, epoch, \
                extract(epoch from last_seen_at)::bigint AS last_seen_at \
           FROM federation_peer ORDER BY node_id",
    )
    .fetch_all(pool)
    .await?;

    rows.iter()
        .map(|row| {
            let state_name: String = row.try_get("trust_state")?;
            let trust_state = TrustState::from_name(&state_name).ok_or_else(|| {
                ApiError::FederationInvariant(format!("trust_state ilegível: {state_name}"))
            })?;
            Ok(PeerRecord {
                peer_id: row.try_get("peer_id")?,
                node_id: row.try_get("node_id")?,
                public_key: row.try_get("public_key")?,
                cluster: row.try_get("cluster")?,
                region: row.try_get("region")?,
                trust_state,
                endpoints: row.try_get("endpoints")?,
                software_version: row.try_get("software_version")?,
                capabilities: row.try_get("capabilities")?,
                epoch: row.try_get::<i64, _>("epoch")? as u64,
                last_seen_at: row.try_get("last_seen_at")?,
            })
        })
        .collect()
}
