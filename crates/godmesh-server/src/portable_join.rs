//! M1C-B: política configurada e autoridade de ingresso portátil no PostgreSQL.

use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;

use godmesh_federation::join_authority::{
    JoinRevocationReceiptV1, JoinRevocationV1, JoinValidityV1,
};
use godmesh_federation::join_chain::{
    ExpectedJoinIdentity, JoinCertificateV2, JoinChainPolicy, JoinChainV2, JoinIssuerV2,
    JoinLinkV2, VerifiedJoinChain, manifest_digest, verify_join_chain,
};
use godmesh_federation::manifest::MAX_FIELD_LEN;
use godmesh_federation::transport::ALPN_V1_TEXT;
use godmesh_federation::{Genesis, GenesisNode, NodeIdentity, SignedManifest, now_unix};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::watch;

use crate::federation::{GOVERNANCE_ENV, JoinWindow};

const ADVERTISE: &str = "GODMESH_FEDERATION_ADVERTISE_QUIC";
const DEPTH: &str = "GODMESH_FEDERATION_PORTABLE_MAX_DEPTH";
const FRAME: &str = "GODMESH_FEDERATION_PORTABLE_MAX_FRAME_BYTES";
const VALIDITY: &str = "GODMESH_FEDERATION_PORTABLE_MAX_VALIDITY_SECONDS";
const REGION: &str = "GODMESH_FEDERATION_REGION";
const CLUSTER: &str = "GODMESH_FEDERATION_CLUSTER";
const VERSION: &str = "GODMESH_FEDERATION_VERSION";

#[derive(Debug)]
pub enum PortableJoinError {
    Configuration(&'static str),
    Storage(sqlx::Error),
    Disabled,
    NotFound,
    Proof(&'static str),
    Issuer,
    Validity,
    Revoked,
    Revocation,
    Epoch { recorded: i64, presented: i64 },
    State(String),
    Replacement,
    RevocationConflict,
    Capacity,
    Encoding(serde_json::Error),
}

impl std::fmt::Display for PortableJoinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Configuration(detail) => write!(f, "política portátil incompatível: {detail}"),
            other => write!(f, "{other:?}"),
        }
    }
}

impl std::error::Error for PortableJoinError {}

impl From<sqlx::Error> for PortableJoinError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error)
    }
}

impl From<serde_json::Error> for PortableJoinError {
    fn from(error: serde_json::Error) -> Self {
        Self::Encoding(error)
    }
}

#[derive(Clone, Debug)]
pub struct PortableJoinConfig {
    policy: JoinChainPolicy,
    endpoint: String,
    region: String,
    version: String,
    max_validity_seconds: i64,
}

impl PortableJoinConfig {
    /// Governança e pins também pertencem ao ingresso v1. O anúncio ou
    /// qualquer limite portátil inicia a configuração explícita de v2.
    pub fn from_env() -> Result<Option<Self>, PortableJoinError> {
        for name in [
            ADVERTISE,
            DEPTH,
            FRAME,
            VALIDITY,
            REGION,
            CLUSTER,
            VERSION,
            GOVERNANCE_ENV,
            godmesh_federation::GENESIS_ENV,
        ] {
            if matches!(std::env::var(name), Err(std::env::VarError::NotUnicode(_))) {
                return Err(PortableJoinError::Configuration(name));
            }
        }
        Self::from_env_with(|name| std::env::var(name).ok())
    }

    pub fn from_env_with(
        get: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, PortableJoinError> {
        if [ADVERTISE, DEPTH, FRAME, VALIDITY]
            .into_iter()
            .all(|name| get(name).is_none())
        {
            return Ok(None);
        }
        let required = |name| get(name).ok_or(PortableJoinError::Configuration(name));
        let governance = required(GOVERNANCE_ENV)?;
        let cluster = governance
            .strip_prefix("single_approver:")
            .filter(|cluster| {
                !cluster.is_empty()
                    && cluster.len() <= MAX_FIELD_LEN
                    && !cluster.contains(|c: char| c == ':' || c.is_control() || c.is_whitespace())
            })
            .ok_or(PortableJoinError::Configuration(GOVERNANCE_ENV))?;
        if get(CLUSTER).is_some_and(|configured| configured.trim() != cluster) {
            return Err(PortableJoinError::Configuration(CLUSTER));
        }
        let genesis = Genesis::parse(&required(godmesh_federation::GENESIS_ENV)?)
            .map_err(|_| PortableJoinError::Configuration(godmesh_federation::GENESIS_ENV))?;
        if genesis.is_empty()
            || genesis
                .nodes()
                .iter()
                .any(|pin| pin.public_key.bytes().any(|b| b.is_ascii_uppercase()))
        {
            return Err(PortableJoinError::Configuration(
                godmesh_federation::GENESIS_ENV,
            ));
        }
        let endpoint = required(ADVERTISE)?;
        if !public_endpoint(&endpoint) {
            return Err(PortableJoinError::Configuration(ADVERTISE));
        }
        let region = required(REGION)?.trim().to_owned();
        if region.trim().is_empty()
            || region.len() > MAX_FIELD_LEN
            || region.chars().any(char::is_control)
        {
            return Err(PortableJoinError::Configuration(REGION));
        }
        let version = get(VERSION)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
        if version.len() > MAX_FIELD_LEN || version.chars().any(char::is_control) {
            return Err(PortableJoinError::Configuration(VERSION));
        }
        let positive = |name| {
            let value = required(name)?;
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(PortableJoinError::Configuration(name));
            }
            Ok(value)
        };
        let limit = |name| {
            positive(name)?
                .parse::<NonZeroUsize>()
                .map_err(|_| PortableJoinError::Configuration(name))
        };
        let max_validity_seconds = positive(VALIDITY)?
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or(PortableJoinError::Configuration(VALIDITY))?;
        Ok(Some(Self {
            policy: JoinChainPolicy {
                cluster: cluster.to_owned(),
                roots: genesis.nodes().to_vec(),
                max_depth: limit(DEPTH)?,
                max_frame_bytes: limit(FRAME)?,
            },
            endpoint,
            region,
            version,
            max_validity_seconds,
        }))
    }

    fn pin(&self, node_id: &str) -> Option<&GenesisNode> {
        self.policy.roots.iter().find(|pin| pin.node_id == node_id)
    }
}

fn public_endpoint(endpoint: &str) -> bool {
    let Some(address) = endpoint.strip_prefix("quic://") else {
        return false;
    };
    let Some((_, port)) = address.rsplit_once(':') else {
        return false;
    };
    if !port.bytes().all(|byte| byte.is_ascii_digit()) || address.contains('%') {
        return false;
    }
    address.parse::<SocketAddr>().is_ok_and(|socket| {
        if socket.port() == 0 {
            return false;
        }
        let ip = socket.ip();
        let ip = match ip {
            std::net::IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, std::net::IpAddr::V4),
            _ => ip,
        };
        !ip.is_unspecified() && !ip.is_multicast()
    })
}

pub struct PortableJoin {
    config: PortableJoinConfig,
    pool: PgPool,
    identity: Arc<NodeIdentity>,
    trust_changes: watch::Sender<u64>,
}

fn cluster_lock_name(cluster: &str) -> String {
    format!("godmesh.federation.portable.v2:{cluster}")
}

/// Evidência de que a transação mantém o lock M1C para este cluster.
pub struct ClusterGuard<'tx, 'db> {
    tx: &'tx mut Transaction<'db, Postgres>,
    cluster: String,
}

impl<'db> ClusterGuard<'_, 'db> {
    pub fn cluster(&self) -> &str {
        &self.cluster
    }

    /// Executa SQL do efetor na mesma transação. Reserve comandos de controle
    /// transacional para o handle original, depois de liberar o empréstimo.
    pub fn connection(&mut self) -> &mut sqlx::PgConnection {
        self.tx
    }

    pub(crate) fn transaction(&mut self) -> &mut Transaction<'db, Postgres> {
        self.tx
    }
}

impl PortableJoin {
    pub fn new(config: PortableJoinConfig, pool: PgPool, identity: Arc<NodeIdentity>) -> Self {
        Self {
            config,
            pool,
            identity,
            trust_changes: watch::channel(0).0,
        }
    }

    pub fn cluster(&self) -> &str {
        &self.config.policy.cluster
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn trust_changes(&self) -> watch::Receiver<u64> {
        self.trust_changes.subscribe()
    }

    async fn commit_trust_change(
        &self,
        tx: Transaction<'static, Postgres>,
    ) -> Result<(), PortableJoinError> {
        let changes = self.trust_changes.clone();
        // Uma requisição pode expirar durante COMMIT. Concluir a notificação
        // junto do resultado do PostgreSQL mesmo se o handler for cancelado.
        tokio::spawn(async move {
            tx.commit().await?;
            changes.send_modify(|version| *version = version.wrapping_add(1));
            Ok::<_, sqlx::Error>(())
        })
        .await
        .map_err(|e| sqlx::Error::Protocol(e.to_string()))??;
        Ok(())
    }

    /// Executar após migrar e antes de abrir listeners, mesmo com v2 desativado.
    pub async fn preflight(
        pool: &PgPool,
        config: Option<&PortableJoinConfig>,
    ) -> Result<(), PortableJoinError> {
        let roots: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT cluster, root_node_id, public_key FROM federation_portable_roots",
        )
        .fetch_all(pool)
        .await?;
        for (cluster, root, key) in roots {
            if !config.is_some_and(|cfg| {
                cfg.policy.cluster == cluster
                    && cfg.pin(&root).is_some_and(|pin| pin.public_key == key)
            }) {
                return Err(PortableJoinError::Configuration(
                    "cluster ou pin exigido pelo estado durável",
                ));
            }
        }
        Ok(())
    }

    /// Usar em READ COMMITTED, antes de locks de pedido, identidade, par ou ingest.
    pub async fn lock_cluster(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), PortableJoinError> {
        let read_committed: bool = sqlx::query_scalar(
            "SELECT current_setting('transaction_isolation') = 'read committed'
               FROM pg_advisory_xact_lock(hashtextextended($1, 0))",
        )
        .bind(cluster_lock_name(self.cluster()))
        .fetch_one(&mut **tx)
        .await?;
        if !read_committed {
            // Um snapshot mais antigo pode ignorar uma revogação que detinha
            // o bloqueio quando a transação de autorização começou.
            return Err(PortableJoinError::Configuration(
                "a transação exige READ COMMITTED",
            ));
        }
        Ok(())
    }

    /// Adquire o lock transacional em READ COMMITTED e empresta a transação.
    pub async fn lock_cluster_guard<'tx, 'db>(
        &self,
        tx: &'tx mut Transaction<'db, Postgres>,
    ) -> Result<ClusterGuard<'tx, 'db>, PortableJoinError> {
        self.lock_cluster(tx).await?;
        Ok(ClusterGuard {
            tx,
            cluster: self.cluster().to_owned(),
        })
    }

    async fn begin(&self) -> Result<Transaction<'static, Postgres>, PortableJoinError> {
        let mut tx = self
            .pool
            .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
            .await?;
        self.lock_cluster(&mut tx).await?;
        Ok(tx)
    }

    pub async fn authorize_local(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<i64>, PortableJoinError> {
        let mut guard = self.lock_cluster_guard(tx).await?;
        self.authorize_local_at(&mut guard, now_unix()).await
    }

    /// Autoridade local no segundo amostrado sob o lock do cluster pelo chamador.
    ///
    /// Uma transação crua não serve de evidência, ainda que a sessão possua lock.
    /// ```compile_fail
    /// # use godmesh_server::portable_join::PortableJoin;
    /// # use sqlx::{Postgres, Transaction};
    /// # async fn run(portable: &PortableJoin, tx: &mut Transaction<'_, Postgres>) {
    /// portable.authorize_local_at(tx, 0).await.unwrap();
    /// # }
    /// ```
    pub async fn authorize_local_at(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        now: i64,
    ) -> Result<Option<i64>, PortableJoinError> {
        if guard.cluster() != self.cluster() {
            return Err(PortableJoinError::Configuration("guard de outro cluster"));
        }
        let tx = guard.transaction();
        let id = self.identity.node_id();
        let key = self.identity.public_key_hex();
        if self.config.pin(id).is_some_and(|pin| pin.public_key == key) {
            return Ok(None);
        }
        let proof = self
            .load_import(tx, id)
            .await?
            .ok_or(PortableJoinError::Issuer)?;
        if proof.peer_node_id.is_some() {
            return Err(PortableJoinError::Issuer);
        }
        let expiry = self.active_import(tx, &proof, id, &key, now).await?;
        Ok(Some(expiry))
    }

    pub async fn authorize_peer(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        node_id: &str,
        public_key: &str,
    ) -> Result<Option<i64>, PortableJoinError> {
        let mut guard = self.lock_cluster_guard(tx).await?;
        self.authorize_peer_at(&mut guard, node_id, public_key, now_unix())
            .await
    }

    /// Autoridade do par no segundo amostrado sob o lock do cluster pelo chamador.
    pub async fn authorize_peer_at(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        node_id: &str,
        public_key: &str,
        now: i64,
    ) -> Result<Option<i64>, PortableJoinError> {
        if guard.cluster() != self.cluster() {
            return Err(PortableJoinError::Configuration("guard de outro cluster"));
        }
        let tx = guard.transaction();
        let proof = self
            .load_import(tx, node_id)
            .await?
            .ok_or(PortableJoinError::Issuer)?;
        if proof.peer_node_id.as_deref() != Some(node_id) {
            return Err(PortableJoinError::Issuer);
        }
        let peer: Option<(String, String, String)> = sqlx::query_as(
            "SELECT trust_state, public_key, cluster FROM federation_peer WHERE node_id = $1 FOR SHARE",
        ).bind(node_id).fetch_optional(&mut **tx).await?;
        if !matches!(peer, Some((state, key, cluster)) if state == "authorized" && key == public_key && cluster == self.cluster())
        {
            return Err(PortableJoinError::Issuer);
        }
        if proof.origin == "pinned_genesis" {
            return self
                .config
                .pin(node_id)
                .filter(|pin| pin.public_key == public_key && proof.public_key == public_key)
                .map(|_| None)
                .ok_or(PortableJoinError::Issuer);
        }
        Ok(Some(
            self.active_import(tx, &proof, node_id, public_key, now)
                .await?,
        ))
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct JoinSignatureRequest {
    pub join_request_id: String,
    pub manifest: SignedManifest,
    pub can_approve: bool,
    pub not_before: i64,
    pub expires_at: i64,
    pub epoch: u64,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChainEnvelope {
    pub chain_json: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChainImportEnvelope {
    pub expected_node_id: String,
    pub expected_public_key: String,
    pub chain_json: String,
    pub validity: JoinValidityV1,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RevocationEnvelope {
    pub target_chain_json: String,
    pub revocation: JoinRevocationV1,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RevocationImportEnvelope {
    pub target_chain_json: String,
    pub revocation: JoinRevocationV1,
    pub receipt: JoinRevocationReceiptV1,
}

type ImportRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<Vec<String>>,
    Option<String>,
    i64,
);
struct Imported {
    origin: String,
    public_key: String,
    peer_node_id: Option<String>,
    chain_json: Option<String>,
    digests: Option<Vec<String>>,
    validity: Option<String>,
    issued_at: i64,
}

struct FramedChain {
    chain: JoinChainV2,
    digests: Vec<String>,
    root_node_id: String,
}

impl FramedChain {
    fn leaf(&self) -> &JoinLinkV2 {
        self.chain
            .links
            .last()
            .expect("cadeia enquadrada é não vazia")
    }

    fn expected(&self) -> ExpectedJoinIdentity {
        ExpectedJoinIdentity {
            node_id: self.leaf().certificate.candidate_node_id.clone(),
            public_key: self.leaf().certificate.candidate_public_key.clone(),
        }
    }

    fn signer_node_id(&self) -> &str {
        self.chain
            .links
            .iter()
            .rev()
            .nth(1)
            .map_or(&self.root_node_id, |parent| {
                &parent.certificate.candidate_node_id
            })
    }
}

fn profile(manifest: &SignedManifest) -> Result<(), PortableJoinError> {
    if !manifest.capabilities.is_empty()
        || !matches!(manifest.endpoints.as_slice(), [endpoint] if public_endpoint(endpoint))
        || manifest.alpn != ALPN_V1_TEXT
        || i64::try_from(manifest.epoch).is_err()
        || manifest.to_frame().is_err()
    {
        return Err(PortableJoinError::Proof("manifest_profile"));
    }
    Ok(())
}

fn fresh(manifest: &SignedManifest, window: JoinWindow, now: i64) -> Result<(), PortableJoinError> {
    profile(manifest)?;
    manifest
        .verify_fresh(now, window.max_age_seconds)
        .map_err(|_| PortableJoinError::Proof("manifest_stale"))
}

fn envelope_capacity(
    chain_json: &str,
    cluster: &str,
    max_body: usize,
) -> Result<(), PortableJoinError> {
    // Medir os próprios envelopes usados no fio com as maiores larguras de
    // cada campo ainda desconhecido. i64::MIN tem a representação mais longa.
    let node = format!("gmn_{}", "f".repeat(64));
    let hex = "f".repeat(64);
    let signature = "f".repeat(128);
    let validity = JoinValidityV1 {
        cluster: cluster.into(),
        root_node_id: node.clone(),
        leaf_digest: hex.clone(),
        issued_at: i64::MIN,
        expires_at: i64::MIN,
        signature: signature.clone(),
    };
    let revocation = JoinRevocationV1 {
        cluster: cluster.into(),
        root_node_id: node.clone(),
        target_digest: hex.clone(),
        revoker_node_id: node.clone(),
        epoch: i64::MAX as u64,
        signature: signature.clone(),
    };
    let receipt = JoinRevocationReceiptV1 {
        cluster: cluster.into(),
        root_node_id: node.clone(),
        revocation_digest: hex.clone(),
        accepted_at: i64::MIN,
        signature,
    };
    let sizes = [
        serde_json::to_vec(&ChainEnvelope {
            chain_json: chain_json.into(),
        })?
        .len(),
        serde_json::to_vec(&ChainImportEnvelope {
            expected_node_id: node,
            expected_public_key: hex,
            chain_json: chain_json.into(),
            validity,
        })?
        .len(),
        serde_json::to_vec(&RevocationEnvelope {
            target_chain_json: chain_json.into(),
            revocation: revocation.clone(),
        })?
        .len(),
        serde_json::to_vec(&RevocationImportEnvelope {
            target_chain_json: chain_json.into(),
            revocation,
            receipt,
        })?
        .len(),
    ];
    if sizes.into_iter().any(|size| size > max_body) {
        return Err(PortableJoinError::Capacity);
    }
    Ok(())
}

impl PortableJoin {
    pub fn local_manifest(&self) -> Result<SignedManifest, PortableJoinError> {
        let mut manifest = SignedManifest {
            node_id: self.identity.node_id().into(),
            public_key: self.identity.public_key_hex(),
            cluster: self.cluster().into(),
            region: self.config.region.clone(),
            software_version: self.config.version.clone(),
            alpn: ALPN_V1_TEXT.into(),
            capabilities: vec![],
            endpoints: vec![self.config.endpoint.clone()],
            epoch: 1,
            issued_at: now_unix(),
            signature: String::new(),
        };
        manifest.sign(&self.identity);
        profile(&manifest)?;
        Ok(manifest)
    }

    fn frame(&self, raw: &str) -> Result<FramedChain, PortableJoinError> {
        if raw.len() > self.config.policy.max_frame_bytes.get() {
            return Err(PortableJoinError::Proof("frame_too_large"));
        }
        if !raw.trim_start().starts_with('{') {
            return Err(PortableJoinError::Proof("malformed_frame"));
        }
        let chain: JoinChainV2 =
            serde_json::from_str(raw).map_err(|_| PortableJoinError::Proof("malformed_frame"))?;
        if chain.version != 2 || chain.links.is_empty() {
            return Err(PortableJoinError::Proof("invalid_chain"));
        }
        if chain.links.len() > self.config.policy.max_depth.get() {
            return Err(PortableJoinError::Proof("depth_exceeded"));
        }
        let JoinIssuerV2::Genesis { node_id } = &chain.links[0].certificate.issuer else {
            return Err(PortableJoinError::Proof("invalid_root"));
        };
        let root_node_id = node_id.clone();
        let digests = chain
            .links
            .iter()
            .map(|link| {
                let signature = &link.certificate.signature;
                if signature.len() != 128
                    || !signature
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(PortableJoinError::Proof("invalid_signature"));
                }
                link.certificate
                    .digest()
                    .map_err(|_| PortableJoinError::Proof("invalid_signature"))
            })
            .collect::<Result<_, _>>()?;
        Ok(FramedChain {
            chain,
            digests,
            root_node_id,
        })
    }

    fn verify(
        &self,
        frame: &FramedChain,
        raw: &str,
        expected: &ExpectedJoinIdentity,
        now: i64,
    ) -> Result<VerifiedJoinChain, PortableJoinError> {
        use godmesh_federation::join_chain::JoinChainError;
        let verified = verify_join_chain(raw.as_bytes(), expected, now, &self.config.policy)
            .map_err(|error| {
                PortableJoinError::Proof(match error {
                    JoinChainError::FrameTooLarge => "frame_too_large",
                    JoinChainError::DepthExceeded => "depth_exceeded",
                    JoinChainError::MalformedFrame => "malformed_frame",
                    JoinChainError::Expired(_) => "expired",
                    JoinChainError::NotYetValid(_) => "not_yet_valid",
                    _ => "invalid_chain",
                })
            })?;
        for link in &frame.chain.links {
            profile(&link.manifest)?;
        }
        Ok(verified)
    }

    fn require_root(&self, chain: &FramedChain) -> Result<(), PortableJoinError> {
        if chain.root_node_id != self.identity.node_id()
            || !self
                .config
                .pin(&chain.root_node_id)
                .is_some_and(|pin| pin.public_key == self.identity.public_key_hex())
        {
            return Err(PortableJoinError::Issuer);
        }
        Ok(())
    }

    async fn retain_root(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        root: &GenesisNode,
    ) -> Result<(), PortableJoinError> {
        sqlx::query(
            "INSERT INTO federation_portable_roots (cluster, root_node_id, public_key)
             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(self.cluster())
        .bind(&root.node_id)
        .bind(&root.public_key)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn load_import(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        id: &str,
    ) -> Result<Option<Imported>, PortableJoinError> {
        let record: Option<ImportRow> = sqlx::query_as(
            "SELECT origin, public_key, peer_node_id, chain_json, digests, validity::text, issued_at
             FROM federation_portable_imports WHERE cluster = $1 AND node_id = $2",
        ).bind(self.cluster()).bind(id).fetch_optional(&mut **tx).await?;
        Ok(record.map(
            |(origin, public_key, peer_node_id, chain_json, digests, validity, issued_at)| {
                Imported {
                    origin,
                    public_key,
                    peer_node_id,
                    chain_json,
                    digests,
                    validity,
                    issued_at,
                }
            },
        ))
    }

    async fn active_import(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        proof: &Imported,
        id: &str,
        key: &str,
        now: i64,
    ) -> Result<i64, PortableJoinError> {
        if proof.origin != "portable_chain" || proof.public_key != key {
            return Err(PortableJoinError::Issuer);
        }
        let raw = proof
            .chain_json
            .as_deref()
            .ok_or(PortableJoinError::Issuer)?;
        let frame = self.frame(raw)?;
        let expected = ExpectedJoinIdentity {
            node_id: id.to_owned(),
            public_key: key.to_owned(),
        };
        let verified = self.verify(&frame, raw, &expected, now)?;
        self.require_unrevoked(tx, &frame).await?;
        let validity: JoinValidityV1 = serde_json::from_str(
            proof
                .validity
                .as_deref()
                .ok_or(PortableJoinError::Validity)?,
        )?;
        self.verify_validity(&validity, &verified, now)?;
        Ok(validity.expires_at)
    }

    fn verify_validity(
        &self,
        validity: &JoinValidityV1,
        verified: &VerifiedJoinChain,
        now: i64,
    ) -> Result<(), PortableJoinError> {
        validity
            .verify_for_chain(
                &verified.root().public_key,
                verified,
                now,
                self.config.max_validity_seconds,
            )
            .map_err(|_| PortableJoinError::Validity)
    }

    async fn require_unrevoked(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        frame: &FramedChain,
    ) -> Result<(), PortableJoinError> {
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM federation_portable_revocations
              WHERE cluster = $1 AND root_node_id = $2 AND target_digest = ANY($3))",
        )
        .bind(self.cluster())
        .bind(&frame.root_node_id)
        .bind(&frame.digests)
        .fetch_one(&mut **tx)
        .await?;
        if blocked {
            return Err(PortableJoinError::Revoked);
        }
        Ok(())
    }

    async fn require_confirmed(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        frame: &FramedChain,
        count: usize,
    ) -> Result<(), PortableJoinError> {
        let confirmed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM federation_portable_decisions
              WHERE cluster = $1 AND root_node_id = $2 AND leaf_digest = ANY($3)",
        )
        .bind(self.cluster())
        .bind(&frame.root_node_id)
        .bind(&frame.digests[..count])
        .fetch_one(&mut **tx)
        .await?;
        if usize::try_from(confirmed).ok() != Some(count) {
            return Err(PortableJoinError::Issuer);
        }
        Ok(())
    }

    async fn require_parent_authority(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        frame: &FramedChain,
        now: i64,
    ) -> Result<(), PortableJoinError> {
        if frame.digests.len() == 1 {
            return Ok(());
        }
        // Uma raiz pode ter confirmado o pai sem importá-lo como par.
        // Se o par existe e foi revogado explicitamente, uma vigência anterior
        // nunca habilita uma nova decisão assinada por ele.
        let trust_state: Option<String> = sqlx::query_scalar(
            "SELECT trust_state FROM federation_peer WHERE node_id = $1 FOR SHARE",
        )
        .bind(frame.signer_node_id())
        .fetch_optional(&mut **tx)
        .await?;
        if trust_state.as_deref() == Some("revoked") {
            return Err(PortableJoinError::Issuer);
        }
        let parent = &frame.digests[frame.digests.len() - 2];
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM federation_portable_validity
              WHERE cluster = $1 AND root_node_id = $2 AND leaf_digest = $3
                AND issued_at <= $4 AND expires_at > $4)",
        )
        .bind(self.cluster())
        .bind(&frame.root_node_id)
        .bind(parent)
        .bind(now)
        .fetch_one(&mut **tx)
        .await?;
        if !active {
            return Err(PortableJoinError::Issuer);
        }
        Ok(())
    }

    async fn advance_epoch(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        root: &str,
        signer: &str,
        presented: u64,
    ) -> Result<(), PortableJoinError> {
        let presented =
            i64::try_from(presented).map_err(|_| PortableJoinError::Proof("invalid_epoch"))?;
        let result = sqlx::query(
            "INSERT INTO federation_portable_epoch (cluster, root_node_id, signer_node_id, epoch)
             VALUES ($1, $2, $3, $4) ON CONFLICT (cluster, root_node_id, signer_node_id)
             DO UPDATE SET epoch = EXCLUDED.epoch WHERE federation_portable_epoch.epoch <= EXCLUDED.epoch",
        ).bind(self.cluster()).bind(root).bind(signer).bind(presented).execute(&mut **tx).await?;
        if result.rows_affected() == 0 {
            let recorded = sqlx::query_scalar(
                "SELECT epoch FROM federation_portable_epoch
                  WHERE cluster = $1 AND root_node_id = $2 AND signer_node_id = $3",
            )
            .bind(self.cluster())
            .bind(root)
            .bind(signer)
            .fetch_one(&mut **tx)
            .await?;
            return Err(PortableJoinError::Epoch {
                recorded,
                presented,
            });
        }
        Ok(())
    }

    pub async fn sign_offer(
        &self,
        request: JoinSignatureRequest,
        window: JoinWindow,
        max_body: usize,
    ) -> Result<String, PortableJoinError> {
        let mut tx = self.begin().await?;
        let now = now_unix();
        fresh(&request.manifest, window, now)?;
        let id = self.identity.node_id();
        let key = self.identity.public_key_hex();
        let (mut links, issuer) = if self.config.pin(id).is_some_and(|pin| pin.public_key == key) {
            (Vec::new(), JoinIssuerV2::Genesis { node_id: id.into() })
        } else {
            let proof = self
                .load_import(&mut tx, id)
                .await?
                .ok_or(PortableJoinError::Issuer)?;
            if proof.peer_node_id.is_some() {
                return Err(PortableJoinError::Issuer);
            }
            self.active_import(&mut tx, &proof, id, &key, now).await?;
            let chain = self.frame(
                proof
                    .chain_json
                    .as_deref()
                    .ok_or(PortableJoinError::Issuer)?,
            )?;
            if !chain.leaf().certificate.can_approve {
                return Err(PortableJoinError::Issuer);
            }
            let digest = chain.digests.last().expect("cadeia não vazia").clone();
            (chain.chain.links, JoinIssuerV2::Parent { digest })
        };
        let mut certificate = JoinCertificateV2 {
            join_request_id: request.join_request_id,
            cluster: self.cluster().into(),
            candidate_node_id: request.manifest.node_id.clone(),
            candidate_public_key: request.manifest.public_key.clone(),
            manifest_digest: manifest_digest(&request.manifest)
                .map_err(|_| PortableJoinError::Proof("manifest_invalid"))?,
            issuer,
            can_approve: request.can_approve,
            not_before: request.not_before,
            expires_at: request.expires_at,
            epoch: request.epoch,
            signature: String::new(),
        };
        certificate.sign(&self.identity);
        links.push(JoinLinkV2 {
            certificate,
            manifest: request.manifest,
        });
        let raw = serde_json::to_string(&JoinChainV2 { version: 2, links })?;
        let frame = self.frame(&raw)?;
        // O filho pode iniciar no futuro, mas o pai precisa estar ativo no
        // instante da assinatura. Na raiz, confirmar exige a cadeia já ativa.
        self.verify(&frame, &raw, &frame.expected(), now.max(request.not_before))?;
        envelope_capacity(&raw, self.cluster(), max_body)?;
        tx.commit().await?;
        Ok(raw)
    }

    pub async fn approve(
        &self,
        request_id: &str,
        raw: &str,
        approved_by: &str,
        window: JoinWindow,
        max_body: usize,
    ) -> Result<(bool, String), PortableJoinError> {
        let frame = self.frame(raw)?;
        self.require_root(&frame)?;
        let leaf = &frame.leaf().certificate;
        if leaf.join_request_id != request_id || leaf.cluster != self.cluster() {
            return Err(PortableJoinError::Proof("request_binding"));
        }
        let mut tx = self.begin().await?;
        let (state, node_id, stored): (String, String, String) = sqlx::query_as(
            "SELECT state, node_id, manifest::text FROM federation_join_request WHERE join_request_id = $1 FOR UPDATE",
        ).bind(request_id).fetch_optional(&mut *tx).await?.ok_or(PortableJoinError::NotFound)?;
        if let Some((digests, original)) = sqlx::query_as::<_, (Vec<String>, String)>(
            "SELECT digests, chain_json FROM federation_portable_decisions
              WHERE join_request_id = $1 AND cluster = $2 AND root_node_id = $3",
        )
        .bind(request_id)
        .bind(self.cluster())
        .bind(&frame.root_node_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            if digests == frame.digests {
                return Ok((false, original));
            }
            return Err(PortableJoinError::State("approved".into()));
        }
        if state != "pending" {
            return Err(PortableJoinError::State(state));
        }
        let manifest: SignedManifest = serde_json::from_str(&stored)?;
        let now = now_unix();
        fresh(&manifest, window, now)?;
        let expected = ExpectedJoinIdentity {
            node_id,
            public_key: manifest.public_key.clone(),
        };
        let verified = self.verify(&frame, raw, &expected, now)?;
        if frame.leaf().manifest != manifest {
            return Err(PortableJoinError::Proof("request_binding"));
        }
        self.require_unrevoked(&mut tx, &frame).await?;
        self.require_confirmed(&mut tx, &frame, frame.digests.len() - 1)
            .await?;
        self.require_parent_authority(&mut tx, &frame, now).await?;
        let compact = serde_json::to_string(&frame.chain)?;
        if compact.len() > self.config.policy.max_frame_bytes.get() {
            return Err(PortableJoinError::Proof("frame_too_large"));
        }
        envelope_capacity(&compact, self.cluster(), max_body)?;
        self.retain_root(&mut tx, verified.root()).await?;
        self.advance_epoch(
            &mut tx,
            &frame.root_node_id,
            frame.signer_node_id(),
            leaf.epoch,
        )
        .await?;
        // Uma PK por pedido também protege escolhas entre raízes distintas.
        let result = sqlx::query(
            "INSERT INTO federation_portable_decisions
             (join_request_id, cluster, root_node_id, signer_node_id, leaf_digest, digests, chain_json, decided_at, approved_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (join_request_id) DO NOTHING",
        ).bind(request_id).bind(self.cluster()).bind(&frame.root_node_id).bind(frame.signer_node_id())
            .bind(verified.leaf_digest()).bind(&frame.digests).bind(&compact).bind(now).bind(approved_by)
            .execute(&mut *tx).await?;
        if result.rows_affected() == 0 {
            return Err(PortableJoinError::State("approved".into()));
        }
        tx.commit().await?;
        Ok((true, compact))
    }

    pub async fn proof(&self, request_id: &str) -> Result<String, PortableJoinError> {
        sqlx::query_scalar(
            "SELECT chain_json FROM federation_portable_decisions
              WHERE join_request_id = $1 AND cluster = $2 AND root_node_id = $3",
        )
        .bind(request_id)
        .bind(self.cluster())
        .bind(self.identity.node_id())
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PortableJoinError::NotFound)
    }

    pub async fn issue_validity(&self, raw: &str) -> Result<JoinValidityV1, PortableJoinError> {
        let frame = self.frame(raw)?;
        self.require_root(&frame)?;
        let mut tx = self.begin().await?;
        let now = now_unix();
        let verified = self.verify(&frame, raw, &frame.expected(), now)?;
        self.require_unrevoked(&mut tx, &frame).await?;
        self.require_confirmed(&mut tx, &frame, frame.digests.len())
            .await?;
        let expires_at = now
            .checked_add(self.config.max_validity_seconds)
            .ok_or(PortableJoinError::Validity)?
            .min(verified.window().1);
        if expires_at <= now {
            return Err(PortableJoinError::Validity);
        }
        let mut validity = JoinValidityV1 {
            cluster: self.cluster().into(),
            root_node_id: frame.root_node_id.clone(),
            leaf_digest: verified.leaf_digest().into(),
            issued_at: now,
            expires_at,
            signature: String::new(),
        };
        validity.sign(&self.identity);
        sqlx::query(
            "INSERT INTO federation_portable_validity (cluster, root_node_id, leaf_digest, issued_at, expires_at, validity)
             VALUES ($1, $2, $3, $4, $5, $6::jsonb)
             ON CONFLICT (cluster, root_node_id, leaf_digest, issued_at)
             DO UPDATE SET expires_at = EXCLUDED.expires_at, validity = EXCLUDED.validity",
        ).bind(self.cluster()).bind(&frame.root_node_id).bind(verified.leaf_digest())
            .bind(now).bind(expires_at).bind(serde_json::to_string(&validity)?).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(validity)
    }
}

impl PortableJoin {
    async fn save_peer(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        manifest: &SignedManifest,
    ) -> Result<(), PortableJoinError> {
        let epoch =
            i64::try_from(manifest.epoch).map_err(|_| PortableJoinError::Proof("invalid_epoch"))?;
        let result = sqlx::query(
            "INSERT INTO federation_peer
             (node_id, peer_id, public_key, cluster, region, trust_state, endpoints, software_version, epoch)
             VALUES ($1, $1, $2, $3, $4, 'authorized', $5, $6, $7)
             ON CONFLICT (node_id) DO UPDATE SET
                region = EXCLUDED.region,
                trust_state = CASE WHEN federation_peer.trust_state = 'revoked' THEN 'revoked' ELSE 'authorized' END,
                endpoints = EXCLUDED.endpoints, software_version = EXCLUDED.software_version,
                capabilities = EXCLUDED.capabilities, epoch = EXCLUDED.epoch, updated_at = now()
             WHERE federation_peer.cluster = EXCLUDED.cluster AND federation_peer.public_key = EXCLUDED.public_key",
        ).bind(&manifest.node_id).bind(&manifest.public_key).bind(self.cluster()).bind(&manifest.region)
            .bind(&manifest.endpoints).bind(&manifest.software_version).bind(epoch).execute(&mut **tx).await?;
        if result.rows_affected() == 0 {
            return Err(PortableJoinError::Replacement);
        }
        Ok(())
    }

    pub async fn import_genesis(
        &self,
        manifest: &SignedManifest,
        window: JoinWindow,
    ) -> Result<bool, PortableJoinError> {
        let mut tx = self.begin().await?;
        let now = now_unix();
        fresh(manifest, window, now)?;
        let root = self
            .config
            .pin(&manifest.node_id)
            .filter(|pin| {
                pin.public_key == manifest.public_key && manifest.cluster == self.cluster()
            })
            .ok_or(PortableJoinError::Issuer)?;
        if manifest.node_id == self.identity.node_id() {
            return Err(PortableJoinError::Issuer);
        }
        let existing = self.load_import(&mut tx, &manifest.node_id).await?;
        if let Some(record) = &existing {
            if record.origin != "pinned_genesis" {
                return Err(PortableJoinError::Replacement);
            }
            if manifest.issued_at < record.issued_at {
                return Err(PortableJoinError::Proof("manifest_stale"));
            }
            if manifest.issued_at == record.issued_at {
                return Ok(false);
            }
        }
        self.retain_root(&mut tx, root).await?;
        self.save_peer(&mut tx, manifest).await?;
        sqlx::query(
            "INSERT INTO federation_portable_imports
             (cluster, node_id, public_key, root_node_id, origin, peer_node_id, manifest, issued_at)
             VALUES ($1, $2, $3, $2, 'pinned_genesis', $2, $4::jsonb, $5)
             ON CONFLICT (cluster, node_id) DO UPDATE SET manifest = EXCLUDED.manifest, issued_at = EXCLUDED.issued_at",
        ).bind(self.cluster()).bind(&manifest.node_id).bind(&manifest.public_key)
            .bind(serde_json::to_string(manifest)?).bind(manifest.issued_at).execute(&mut *tx).await?;
        self.commit_trust_change(tx).await?;
        Ok(existing.is_none())
    }

    pub async fn import_chain(
        &self,
        expected_node_id: &str,
        expected_public_key: &str,
        raw: &str,
        validity: &JoinValidityV1,
    ) -> Result<bool, PortableJoinError> {
        let frame = self.frame(raw)?;
        let local = expected_node_id == self.identity.node_id();
        if local && expected_public_key != self.identity.public_key_hex() {
            return Err(PortableJoinError::Issuer);
        }
        let expected = ExpectedJoinIdentity {
            node_id: expected_node_id.into(),
            public_key: expected_public_key.into(),
        };
        let mut tx = self.begin().await?;
        let now = now_unix();
        let verified = self.verify(&frame, raw, &expected, now)?;
        self.require_unrevoked(&mut tx, &frame).await?;
        self.verify_validity(validity, &verified, now)?;
        let existing = self.load_import(&mut tx, expected_node_id).await?;
        if let Some(record) = &existing {
            if record.origin != "portable_chain" {
                return Err(PortableJoinError::Replacement);
            }
            if record.digests.as_ref() == Some(&frame.digests) {
                if validity.issued_at < record.issued_at {
                    return Err(PortableJoinError::Validity);
                }
                if validity.issued_at == record.issued_at {
                    return Ok(false);
                }
            } else {
                match self
                    .active_import(&mut tx, record, expected_node_id, expected_public_key, now)
                    .await
                {
                    Ok(_) => return Err(PortableJoinError::Replacement),
                    Err(
                        PortableJoinError::Validity
                        | PortableJoinError::Revoked
                        | PortableJoinError::Proof("expired"),
                    ) => {}
                    Err(error) => return Err(error),
                }
            }
        }
        let manifest = &frame.leaf().manifest;
        self.retain_root(&mut tx, verified.root()).await?;
        if !local {
            self.save_peer(&mut tx, manifest).await?;
        }
        sqlx::query(
            "INSERT INTO federation_portable_imports
             (cluster, node_id, public_key, root_node_id, origin, peer_node_id,
              manifest, chain_json, digests, validity, issued_at, expires_at)
             VALUES ($1, $2, $3, $4, 'portable_chain', $5, $6::jsonb, $7, $8, $9::jsonb, $10, $11)
             ON CONFLICT (cluster, node_id) DO UPDATE SET
                 root_node_id = EXCLUDED.root_node_id, peer_node_id = EXCLUDED.peer_node_id,
                 manifest = EXCLUDED.manifest, chain_json = EXCLUDED.chain_json,
                 digests = EXCLUDED.digests, validity = EXCLUDED.validity,
                 issued_at = EXCLUDED.issued_at, expires_at = EXCLUDED.expires_at",
        )
        .bind(self.cluster())
        .bind(expected_node_id)
        .bind(expected_public_key)
        .bind(&frame.root_node_id)
        .bind((!local).then_some(expected_node_id))
        .bind(serde_json::to_string(manifest)?)
        .bind(raw)
        .bind(&frame.digests)
        .bind(serde_json::to_string(validity)?)
        .bind(validity.issued_at)
        .bind(validity.expires_at)
        .execute(&mut *tx)
        .await?;
        self.commit_trust_change(tx).await?;
        Ok(existing.is_none())
    }

    fn revoker_key<'a>(
        &'a self,
        target: &'a FramedChain,
        revocation: &JoinRevocationV1,
    ) -> Result<(&'a str, bool), PortableJoinError> {
        if let Some(pin) = self.config.pin(&revocation.revoker_node_id) {
            return Ok((&pin.public_key, false));
        }
        target
            .chain
            .links
            .iter()
            .rev()
            .nth(1)
            .filter(|parent| {
                parent.certificate.candidate_node_id == revocation.revoker_node_id
                    && parent.certificate.can_approve
            })
            .map(|parent| (parent.certificate.candidate_public_key.as_str(), true))
            .ok_or(PortableJoinError::Revocation)
    }

    async fn existing_revocation(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        frame: &FramedChain,
        revocation: &JoinRevocationV1,
    ) -> Result<Option<JoinRevocationReceiptV1>, PortableJoinError> {
        if revocation.cluster != self.cluster()
            || revocation.root_node_id != frame.root_node_id
            || Some(&revocation.target_digest) != frame.digests.last()
        {
            return Err(PortableJoinError::Revocation);
        }
        let existing: Option<(String, String)> = sqlx::query_as(
            "SELECT revocation_digest, receipt::text FROM federation_portable_revocations
              WHERE cluster = $1 AND root_node_id = $2 AND target_digest = $3",
        )
        .bind(self.cluster())
        .bind(&frame.root_node_id)
        .bind(&revocation.target_digest)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((recorded, receipt)) = existing else {
            return Ok(None);
        };
        let digest = revocation
            .digest()
            .map_err(|_| PortableJoinError::Revocation)?;
        if digest != recorded {
            return Err(PortableJoinError::RevocationConflict);
        }
        // O digest decodifica hex. O replay também exige a representação canônica.
        if !revocation
            .signature
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(PortableJoinError::Revocation);
        }
        Ok(Some(serde_json::from_str(&receipt)?))
    }

    async fn save_revocation(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        raw: &str,
        verified: &VerifiedJoinChain,
        revocation: &JoinRevocationV1,
        receipt: &JoinRevocationReceiptV1,
    ) -> Result<(), PortableJoinError> {
        self.retain_root(tx, verified.root()).await?;
        // Revogador genesis também é uma dependência durável do recibo.
        if let Some(pin) = self.config.pin(&revocation.revoker_node_id) {
            self.retain_root(tx, pin).await?;
        }
        sqlx::query(
            "INSERT INTO federation_portable_revocations
             (cluster, root_node_id, target_digest, revocation_digest, target_chain_json, revocation, receipt)
             VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::jsonb)",
        ).bind(self.cluster()).bind(&receipt.root_node_id).bind(&revocation.target_digest)
            .bind(&receipt.revocation_digest).bind(raw).bind(serde_json::to_string(revocation)?)
            .bind(serde_json::to_string(receipt)?).execute(&mut **tx).await?;
        Ok(())
    }

    pub async fn revoke(
        &self,
        raw: &str,
        revocation: &JoinRevocationV1,
    ) -> Result<(bool, JoinRevocationReceiptV1), PortableJoinError> {
        let frame = self.frame(raw)?;
        self.require_root(&frame)?;
        let mut tx = self.begin().await?;
        if let Some(receipt) = self
            .existing_revocation(&mut tx, &frame, revocation)
            .await?
        {
            return Ok((false, receipt));
        }
        let now = now_unix();
        let verified = self.verify(&frame, raw, &frame.expected(), now)?;
        self.require_confirmed(&mut tx, &frame, frame.digests.len())
            .await?;
        let (key, delegated) = self.revoker_key(&frame, revocation)?;
        revocation
            .verify_for_chain(key, &verified)
            .map_err(|_| PortableJoinError::Revocation)?;
        if delegated {
            self.require_unrevoked(&mut tx, &frame).await?;
            self.require_parent_authority(&mut tx, &frame, now).await?;
        }
        let mut receipt = JoinRevocationReceiptV1 {
            cluster: self.cluster().into(),
            root_node_id: frame.root_node_id.clone(),
            revocation_digest: revocation
                .digest()
                .map_err(|_| PortableJoinError::Revocation)?,
            accepted_at: now,
            signature: String::new(),
        };
        receipt.sign(&self.identity);
        self.save_revocation(&mut tx, raw, &verified, revocation, &receipt)
            .await?;
        self.advance_epoch(
            &mut tx,
            &frame.root_node_id,
            &revocation.revoker_node_id,
            revocation.epoch,
        )
        .await?;
        self.commit_trust_change(tx).await?;
        Ok((true, receipt))
    }

    pub async fn import_revocation(
        &self,
        raw: &str,
        revocation: &JoinRevocationV1,
        receipt: &JoinRevocationReceiptV1,
    ) -> Result<bool, PortableJoinError> {
        let frame = self.frame(raw)?;
        let mut tx = self.begin().await?;
        let verified = self.verify(&frame, raw, &frame.expected(), receipt.accepted_at)?;
        let (key, _) = self.revoker_key(&frame, revocation)?;
        let evidence = revocation
            .verify_for_chain(key, &verified)
            .map_err(|_| PortableJoinError::Revocation)?;
        receipt
            .verify_for_revocation(&verified.root().public_key, &evidence)
            .map_err(|_| PortableJoinError::Revocation)?;
        if self
            .existing_revocation(&mut tx, &frame, revocation)
            .await?
            .is_some()
        {
            return Ok(false);
        }
        self.save_revocation(&mut tx, raw, &verified, revocation, receipt)
            .await?;
        self.commit_trust_change(tx).await?;
        Ok(true)
    }
}
