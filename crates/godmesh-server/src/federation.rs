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
//! Nenhuma função aqui lê, grava ou registra chave privada: o manifesto e o
//! certificado de aprovação só carregam chave pública, e a identidade de nó do
//! aprovador vive no arquivo do nó que assina, nunca neste processo.
//!
//! ## M1b — a segunda prova
//!
//! Desde a emenda à ADR-071, aprovar ou rejeitar exige **duas** autoridades
//! compostas, e nenhuma das duas substitui a outra:
//!
//! * o grant humano com `federation:admin` autoriza a *operação* (quem disparou);
//! * o [`ApprovalCertificate`] assinado por um nó `authorized` prova a
//!   *autoridade de ingresso* — que quem decidiu é um nó do mesmo cluster.
//!
//! Fontes de autoridade do aprovador, na ordem: par registrado com
//! `trust_state='authorized'` em `federation_peer`, ou nó declarado no
//! [`Genesis`] do operador (ADR-071 §5, bootstrap fora de banda). Sem genesis e
//! sem nenhum par autorizado, **não existe caminho para aprovar** — a resposta
//! diz exatamente isso, em vez de aceitar a primeira chave que aparecer.

use godmesh_federation::Genesis;
use godmesh_federation::SignedManifest;
use godmesh_federation::manifest::{MAX_FIELD_LEN, ManifestError};
use godmesh_federation::peer::{PeerRecord, TrustState};
use godmesh_federation::{ApprovalCertificate, DecisionAction};
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

/// Declaração do operador para o cluster sem votantes.
pub const GOVERNANCE_ENV: &str = "GODMESH_FEDERATION_GOVERNANCE";

/// Configuração local de confiança e governança para decidir ingressos.
#[derive(Debug, Default)]
pub struct DecisionConfig {
    genesis: Genesis,
    single_approver_cluster: Option<String>,
}

impl DecisionConfig {
    /// Valida a declaração explícita do cluster sem votantes.
    ///
    /// # Errors
    /// Modo não suportado ou identificador de cluster malformado.
    pub fn new(genesis: Genesis, governance: Option<&str>) -> Result<Self, String> {
        let single_approver_cluster = match governance {
            None | Some("") => None,
            Some(value) => {
                let cluster = value
                    .strip_prefix("single_approver:")
                    .filter(|cluster| {
                        !cluster.is_empty()
                            && cluster.len() <= MAX_FIELD_LEN
                            && !cluster
                                .contains(|c: char| c == ':' || c.is_control() || c.is_whitespace())
                    })
                    .ok_or_else(|| {
                        format!("{GOVERNANCE_ENV} inválida; esperado single_approver:<cluster>")
                    })?;
                Some(cluster.to_owned())
            }
        };
        Ok(Self {
            genesis,
            single_approver_cluster,
        })
    }

    /// Lê as declarações de confiança do ambiente de serviço.
    ///
    /// # Errors
    /// Genesis ou governança inválida.
    pub fn from_env() -> Result<Self, String> {
        let governance = match std::env::var(GOVERNANCE_ENV) {
            Ok(value) => Some(value),
            Err(std::env::VarError::NotPresent) => None,
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(format!("{GOVERNANCE_ENV} contém bytes não-UTF8"));
            }
        };
        let genesis = Genesis::from_env().map_err(|e| e.to_string())?;
        Self::new(genesis, governance.as_deref())
    }

    /// Quantos nós genesis foram declarados no processo.
    #[must_use]
    pub fn genesis_count(&self) -> usize {
        self.genesis.len()
    }

    fn require_enabled(&self, cluster: &str) -> Result<(), ApiError> {
        if self.single_approver_cluster.as_deref() == Some(cluster) {
            Ok(())
        } else {
            Err(ApiError::FederationGovernanceDisabled)
        }
    }
}

/// Pedido de ingresso como a API devolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
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
    /// Signatário da prova v1. Aprovação portátil é consultada em `/proof`.
    pub approved_by_node: Option<String>,
    /// Assinatura Ed25519 do aprovador sobre o certificado (dado público: é o
    /// que qualquer nó da malha re-verifica contra a chave registrada).
    pub approval_signature: Option<String>,
    /// Geração da identidade do aprovador registrada com a decisão (piso
    /// anti-rollback).
    pub approval_epoch: Option<u64>,
    /// Fim da validade do certificado, em segundos Unix.
    pub approval_expires_at: Option<i64>,
    /// Manifesto assinado como chegou (evidência do que foi pedido).
    pub manifest: SignedManifest,
}

/// Aprovação/rejeição assinada por nó, como a API recebe.
///
/// Corpo próprio (e não o certificado nu) porque a rota é composta de duas
/// autoridades: o certificado é a prova de nó; o escopo humano vem no
/// `Authorization`. Nomear o campo deixa o contrato explícito sobre o que o
/// corpo transporta e abre espaço para decisão futura sem mudar a forma.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDecision {
    /// Prova assinada de que um nó autorizado decidiu sobre este pedido.
    pub approval: ApprovalCertificate,
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
        join_query!("federation_join_request ", $tail)
    };
    ($from:expr, $tail:expr) => {
        concat!(
            "SELECT join_request_id, node_id, epoch, manifest::text AS manifest, ",
            "extract(epoch from requested_at)::bigint AS requested_at, ",
            "extract(epoch from approved_at)::bigint AS approved_at, approved_by, ",
            "approved_by_node, approval_signature, approval_epoch, ",
            "extract(epoch from approval_expires_at)::bigint AS approval_expires_at, state ",
            "FROM ", $from, $tail
        )
    };
}

// O pedido e o certificado legado permanecem intactos. A confirmação v2
// compõe o estado público com o carimbo da decisão e deixa a prova em /proof.
macro_rules! visible_join_query {
    ($tail:expr) => {
        join_query!(
            "(SELECT r.join_request_id, r.node_id, r.epoch, r.manifest, r.requested_at,
                     COALESCE(r.approved_at, to_timestamp(d.decided_at::double precision)) AS approved_at,
                     COALESCE(r.approved_by, d.approved_by) AS approved_by,
                     r.approved_by_node, r.approval_signature, r.approval_epoch, r.approval_expires_at,
                     CASE WHEN d.join_request_id IS NULL THEN r.state ELSE 'approved' END AS state
                FROM federation_join_request r LEFT JOIN federation_portable_decisions d USING (join_request_id)
             ) AS visible ",
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
        approved_by_node: row.try_get("approved_by_node")?,
        approval_signature: row.try_get("approval_signature")?,
        approval_epoch: row
            .try_get::<Option<i64>, _>("approval_epoch")?
            .map(|epoch| {
                u64::try_from(epoch).map_err(|_| {
                    ApiError::FederationInvariant(format!(
                        "approval_epoch negativo para um pedido aprovado: {epoch}"
                    ))
                })
            })
            .transpose()?,
        approval_expires_at: row.try_get("approval_expires_at")?,
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
    let row = sqlx::query(visible_join_query!("WHERE node_id = $1 AND epoch = $2"))
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
            sqlx::query(visible_join_query!(
                "WHERE state = $1 ORDER BY requested_at DESC, join_request_id"
            ))
            .bind(state)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query(visible_join_query!("ORDER BY requested_at DESC, join_request_id"))
                .fetch_all(pool)
                .await?
        }
    };
    rows.iter().map(join_view).collect()
}

/// Estado do aprovador, resolvido a partir do que **este nó** registra como
/// autoridade: `federation_peer` (decisão anterior) ou genesis declarado pelo
/// operador.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ApproverAuthority {
    /// Par no registro: chave e cluster que o banco guarda.
    Registered {
        public_key: String,
        cluster: String,
        trust_state: String,
    },
    /// Nó genesis: autorizado por configuração, fora de banda (ADR-071 §5).
    Genesis { public_key: String },
}

/// Resolve a chave pública registrada de um candidato a aprovador.
///
/// Ordem deliberada: **o registro vence o genesis**. Um nó que estava no
/// bootstrap e depois passou pelo fluxo normal não aprova pela configuração
/// antiga se o registro diz outra coisa — revogação tem de ganhar de bootstrap,
/// ou não seria revogação.
async fn approver_authority(
    tx: &mut sqlx::PgConnection,
    genesis: &Genesis,
    node_id: &str,
    cluster: &str,
) -> Result<ApproverAuthority, ApiError> {
    let row = sqlx::query(
        "SELECT public_key, cluster, trust_state FROM federation_peer \
          WHERE node_id = $1 FOR UPDATE",
    )
    .bind(node_id)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(row) = row {
        return Ok(ApproverAuthority::Registered {
            public_key: row.try_get("public_key")?,
            cluster: row.try_get("cluster")?,
            trust_state: row.try_get("trust_state")?,
        });
    }

    if let Some(public_key) = genesis.public_key_of(node_id) {
        return Ok(ApproverAuthority::Genesis {
            public_key: public_key.to_owned(),
        });
    }

    // Identificador desconhecido. Se além dele não existe **nenhuma** autoridade
    // no cluster do pedido, o problema não é a assinatura do cliente: é o cluster
    // ainda não ter quem aprove. Dizer isso com código próprio evita que o
    // operador caça chave errada onde o que falta é configuração — e a pergunta
    // é por cluster, porque autoridade de ingresso não é global.
    if genesis.is_empty() {
        let alguma: Option<String> = sqlx::query_scalar(
            "SELECT node_id FROM federation_peer \
              WHERE trust_state = 'authorized' AND cluster = $1 LIMIT 1",
        )
        .bind(cluster)
        .fetch_optional(&mut *tx)
        .await?;
        if alguma.is_none() {
            return Err(ApiError::NoAuthorizedApprover);
        }
    }
    Err(ApiError::ApproverNotAuthorized(format!(
        "{node_id} não está em federation_peer nem em GODMESH_FEDERATION_GENESIS"
    )))
}

/// Serializa decisões e a criação de registro para o mesmo nó, inclusive
/// quando o genesis ainda não tem uma linha a travar.
async fn lock_decision_node(tx: &mut sqlx::PgConnection, node_id: &str) -> Result<(), ApiError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('federation.decision:' || $1, 0))")
        .bind(node_id)
        .execute(tx)
        .await?;
    Ok(())
}

/// Verifica a prova de nó que toda decisão de ingresso exige.
///
/// Ordem das checagens — cada uma recusa **antes** de qualquer escrita, e um
/// erro aqui não transiciona estado nenhum:
///
/// 1. autoridade do aprovador: o que o **registro** (`federation_peer`,
///    `trust_state='authorized'`) ou o genesis declarado dizem sobre aquele
///    `node_id`; sem isso não há chave com que verificar, e `no_authorized_approver`
///    nomeia o cluster que ainda não tem aprovador nenhum;
/// 2. assinatura Ed25519 sobre os bytes canônicos, verificada contra essa chave
///    registrada — nunca contra a chave que o corpo auto-declara;
/// 3. o certificado aponta para **este** pedido: assinado sobre
///    `join_request_id`, uma decisão sobre `fjr_A` não aprova `fjr_B`;
/// 4. janela de validade `not_before <= now < expires_at`: certificado
///    expirado é replay de decisão velha, não autoridade viva;
/// 5. cluster: o assinado é o do pedido, e o do par registrado é o do pedido;
/// 6. `epoch` não regressivo em relação às decisões já assinadas pelo nó.
async fn verify_approval_certificate(
    tx: &mut sqlx::PgConnection,
    genesis: &Genesis,
    request: &JoinRequestView,
    certificate: &ApprovalCertificate,
    action: DecisionAction,
    clock: impl FnOnce() -> i64,
) -> Result<(i64, i64), ApiError> {
    // Primeiro a autoridade e a assinatura, depois o que os campos dizem: um
    // campo de um certificado não verificado não é evidência, é alegação. E
    // nada acima disto escreveu no banco — as recusas saem da transação.
    let authority =
        approver_authority(tx, genesis, &certificate.node_id, &request.manifest.cluster).await?;
    let (trusted_key, registered_cluster) = match &authority {
        ApproverAuthority::Registered {
            public_key,
            cluster,
            trust_state,
        } => {
            if trust_state != "authorized" {
                return Err(ApiError::ApproverNotAuthorized(format!(
                    "{} tem trust_state '{trust_state}'; só nó autorizado aprova",
                    certificate.node_id
                )));
            }
            (public_key.clone(), Some(cluster.clone()))
        }
        ApproverAuthority::Genesis { public_key } => (public_key.clone(), None),
    };

    // Assinatura contra a chave **registrada**. Declarar chave diferente da
    // registrada é identidade emprestada, não aprovação.
    certificate
        .verify_for(&trusted_key, action)
        .map_err(|e| match e {
            godmesh_federation::ApprovalError::NodeIdMismatch => {
                ApiError::ApproverNotAuthorized(format!(
                    "certificado de {} não confere com a chave registrada",
                    certificate.node_id
                ))
            }
            other => ApiError::ApprovalSignatureInvalid(other.to_string()),
        })?;

    if certificate.join_request_id != request.join_request_id {
        return Err(ApiError::ApprovalSignatureInvalid(format!(
            "certificado cobre {}, não o pedido {} desta decisão",
            certificate.join_request_id, request.join_request_id
        )));
    }

    // Limites inclusivos de timestamptz do PostgreSQL 16, em segundos Unix.
    const MIN_EXPIRY: i64 = -210_866_803_200;
    const MAX_EXPIRY: i64 = 9_224_318_015_999;
    if !(MIN_EXPIRY..=MAX_EXPIRY).contains(&certificate.expires_at) {
        return Err(ApiError::InvalidRequest(
            "expires_at fora da faixa de timestamptz".to_owned(),
        ));
    }
    let now = clock();
    if let Err(e) = certificate.check_validity(now) {
        return Err(match e {
            godmesh_federation::ApprovalError::Expired { expires_at, .. } => {
                ApiError::ApprovalExpired { expires_at, now }
            }
            godmesh_federation::ApprovalError::NotYetValid { not_before, .. } => {
                ApiError::ApprovalNotYetValid { not_before, now }
            }
            other => ApiError::InvalidRequest(other.to_string()),
        });
    }

    // "no mesmo cluster" (ADR-071 emenda §1): o cluster assinado tem de ser o
    // do pedido; e, quando o aprovador é um par registrado, o cluster dele
    // também.
    if certificate.cluster != request.manifest.cluster {
        return Err(ApiError::ApproverNotAuthorized(format!(
            "certificado assinado no cluster '{}', mas o pedido é do cluster '{}'",
            certificate.cluster, request.manifest.cluster
        )));
    }
    if let Some(cluster) = registered_cluster
        && cluster != request.manifest.cluster
    {
        return Err(ApiError::ApproverNotAuthorized(format!(
            "{} é autorizado no cluster '{cluster}', não no cluster '{}'",
            certificate.node_id, request.manifest.cluster
        )));
    }

    let epoch = i64::try_from(certificate.epoch)
        .map_err(|_| ApiError::InvalidRequest("epoch de decisão excede bigint".to_owned()))?;
    // O advisory lock do aprovador também protege a ausência de uma linha.
    let recorded: Option<i64> = sqlx::query_scalar(
        "SELECT decision_epoch FROM federation_decision_epoch
          WHERE approver_node_id = $1 FOR UPDATE",
    )
    .bind(&certificate.node_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(recorded) = recorded
        && epoch < recorded
    {
        return Err(ApiError::ApprovalEpochRegressed {
            recorded: recorded as u64,
            presented: certificate.epoch,
        });
    }

    Ok((now, epoch))
}

async fn advance_decision_epoch(
    tx: &mut sqlx::PgConnection,
    approver_node_id: &str,
    epoch: i64,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO federation_decision_epoch (approver_node_id, decision_epoch)
         VALUES ($1, $2)
         ON CONFLICT (approver_node_id) DO UPDATE
           SET decision_epoch = EXCLUDED.decision_epoch
         WHERE federation_decision_epoch.decision_epoch < EXCLUDED.decision_epoch",
    )
    .bind(approver_node_id)
    .bind(epoch)
    .execute(tx)
    .await?;
    Ok(())
}

macro_rules! peer_query {
    ($tail:expr) => {
        concat!(
            "SELECT node_id, peer_id, public_key, cluster, region, trust_state, endpoints, ",
            "software_version, capabilities, epoch, ",
            "extract(epoch from last_seen_at)::bigint AS last_seen_at ",
            "FROM federation_peer ",
            $tail
        )
    };
}

/// Aprova um pedido pendente e cria/atualiza o par como `authorized`.
///
/// Idempotente para pedido já aprovado. Pedido `rejected`/`expired` é `409`.
/// Par existente passa pela máquina de estados: `pending → authorized` e
/// `authorized → authorized` valem; `revoked → authorized` **não** vale.
///
/// Antes de qualquer escrita, o [`ApprovalCertificate`] é verificado contra a
/// chave que **este banco ou este operador** registram para o aprovador
/// ([`verify_approval_certificate`]). Recusa de certificado não transiciona
/// estado nenhum: a transação é descartada.
///
/// O pedido, o aprovador e o candidato ficam protegidos até o commit.
/// Os locks por nó são obtidos em ordem estável para aprovações cruzadas.
///
/// # Errors
/// `404` pedido inexistente; `409` estado que não aceita aprovação; `422`
/// certificado inválido/não autorizado/expirado/regressivo; `503` banco.
pub async fn approve_join_request(
    pool: &PgPool,
    config: &DecisionConfig,
    join_request_id: &str,
    approved_by: &str,
    certificate: &ApprovalCertificate,
    clock: impl FnOnce() -> i64,
) -> Result<(JoinRequestView, PeerRecord), ApiError> {
    let mut tx = pool
        .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
        .await?;

    let row = sqlx::query(join_query!("WHERE join_request_id = $1 FOR UPDATE"))
        .bind(join_request_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError::JoinRequestNotFound)?;
    let request = join_view(&row)?;
    config.require_enabled(&request.manifest.cluster)?;
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM federation_portable_decisions WHERE join_request_id = $1)",
    ).bind(join_request_id).fetch_one(&mut *tx).await? {
        return Err(ApiError::FederationGovernanceDisabled);
    }

    let mut nodes = [certificate.node_id.as_str(), request.node_id.as_str()];
    nodes.sort_unstable();
    lock_decision_node(&mut tx, nodes[0]).await?;
    if nodes[0] != nodes[1] {
        lock_decision_node(&mut tx, nodes[1]).await?;
    }
    let (now, approval_epoch) = verify_approval_certificate(
        &mut tx,
        &config.genesis,
        &request,
        certificate,
        DecisionAction::Approve,
        clock,
    )
    .await?;

    if request.state != "pending" && request.state != "approved" {
        return Err(ApiError::JoinRequestStateConflict {
            state: request.state,
        });
    }

    let existing = sqlx::query(peer_query!("WHERE node_id = $1 FOR UPDATE"))
        .bind(&request.node_id)
        .fetch_optional(&mut *tx)
        .await?
        .as_ref()
        .map(peer_record)
        .transpose()?;

    if request.state == "approved" {
        let peer = existing.ok_or_else(|| {
            ApiError::FederationInvariant(format!(
                "pedido {} aprovado sem par",
                request.join_request_id
            ))
        })?;
        if !peer.is_authorized() {
            return Err(ApiError::TrustTransition {
                current: peer.trust_state.as_str().to_owned(),
                next: TrustState::Authorized.as_str().to_owned(),
            });
        }
        tx.commit().await?;
        return Ok((request, peer));
    }

    let mut peer = PeerRecord::from_manifest(&request.manifest, now);
    if let Some(existing) = existing {
        peer.trust_state = existing.trust_state;
    }
    peer.transition_to(TrustState::Authorized)
        .map_err(|e| ApiError::TrustTransition {
            current: e.current.as_str().to_owned(),
            next: e.next.as_str().to_owned(),
        })?;

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

    advance_decision_epoch(&mut tx, &certificate.node_id, approval_epoch).await?;
    // Dias inteiros e segundos residuais evitam converter o instante inteiro
    // em microssegundos via ponto flutuante. UTC independe da sessão.
    sqlx::query(
        "UPDATE federation_join_request
            SET state = 'approved',
                approved_at = to_timestamp($2::double precision),
                approved_by = $3,
                approved_by_node = $4,
                approval_signature = $5,
                approval_epoch = $6,
                approval_expires_at =
                    (date '1970-01-01' + ($7::bigint / 86400)::integer
                     + make_interval(secs => ($7::bigint % 86400)::double precision))
                    AT TIME ZONE 'UTC'
          WHERE join_request_id = $1 AND state = 'pending'",
    )
    .bind(join_request_id)
    .bind(now as f64)
    .bind(approved_by)
    .bind(&certificate.node_id)
    .bind(&certificate.signature)
    .bind(approval_epoch)
    .bind(certificate.expires_at)
    .execute(&mut *tx)
    .await?;

    let row = sqlx::query(join_query!("WHERE join_request_id = $1"))
        .bind(join_request_id)
        .fetch_one(&mut *tx)
        .await?;
    let request = join_view(&row)?;
    tx.commit().await?;
    Ok((request, peer))
}

/// Rejeita um pedido pendente. **Nenhum par é criado.**
///
/// Rejeitar é ação de governança tanto quanto aprovar: exige o escopo humano
/// **e** o certificado assinado por nó autorizado (ADR-071 emenda §1/§2). A
/// diferença é o que fica no banco: `rejected` não tem prova de aprovação, e a
/// migração 0009 recusa essas colunas preenchidas fora de `approved` — quem
/// rejeitou fica na auditoria.
///
/// # Errors
/// `404` pedido inexistente; `409` pedido já decidido; `422` certificado
/// inválido/não autorizado/expirado/regressivo; `503` banco.
pub async fn reject_join_request(
    pool: &PgPool,
    config: &DecisionConfig,
    join_request_id: &str,
    certificate: &ApprovalCertificate,
    clock: impl FnOnce() -> i64,
) -> Result<JoinRequestView, ApiError> {
    let mut tx = pool
        .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
        .await?;

    let row = sqlx::query(join_query!("WHERE join_request_id = $1 FOR UPDATE"))
        .bind(join_request_id)
        .fetch_optional(&mut *tx)
        .await?;

    let Some(row) = row else {
        return Err(ApiError::JoinRequestNotFound);
    };
    let request = join_view(&row)?;

    config.require_enabled(&request.manifest.cluster)?;
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM federation_portable_decisions WHERE join_request_id = $1)",
    ).bind(join_request_id).fetch_one(&mut *tx).await? {
        return Err(ApiError::FederationGovernanceDisabled);
    }
    lock_decision_node(&mut tx, &certificate.node_id).await?;
    let (_, decision_epoch) = verify_approval_certificate(
        &mut tx,
        &config.genesis,
        &request,
        certificate,
        DecisionAction::Reject,
        clock,
    )
    .await?;

    if request.state != "pending" {
        return Err(ApiError::JoinRequestStateConflict {
            state: request.state.clone(),
        });
    }

    advance_decision_epoch(&mut tx, &certificate.node_id, decision_epoch).await?;
    sqlx::query(
        "UPDATE federation_join_request SET state = 'rejected' \
          WHERE join_request_id = $1 AND state = 'pending'",
    )
    .bind(join_request_id)
    .execute(&mut *tx)
    .await?;

    let row = sqlx::query(join_query!("WHERE join_request_id = $1"))
        .bind(join_request_id)
        .fetch_one(&mut *tx)
        .await?;
    let request = join_view(&row)?;
    tx.commit().await?;
    Ok(request)
}

fn peer_record(row: &sqlx::postgres::PgRow) -> Result<PeerRecord, ApiError> {
    let state_name: String = row.try_get("trust_state")?;
    let trust_state = TrustState::from_name(&state_name).ok_or_else(|| {
        ApiError::FederationInvariant(format!("trust_state ilegível no registro: {state_name}"))
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
}

/// Lista pares registrados. `public_key` é dado público.
///
/// # Errors
/// `503` banco, ou estado de confiança persistido ilegível (invariante).
pub async fn list_peers(pool: &PgPool) -> Result<Vec<PeerRecord>, ApiError> {
    let rows = sqlx::query(peer_query!("ORDER BY node_id"))
        .fetch_all(pool)
        .await?;
    rows.iter().map(peer_record).collect()
}
