//! Codec e avaliações puras do perfil M3-A de configuração assinada.

use std::{cmp::Ordering, collections::HashSet, fmt, num::NonZeroUsize};

use serde::{Deserialize, Deserializer, de::Error as _};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

use crate::{
    identity,
    manifest::{MAX_FIELD_LEN, push_field},
};

pub const PROFILE: u64 = 1;
const MAX_INTEGER: u64 = i64::MAX as u64;
pub const NAMESPACE: &str = "federation";
const ENTRY_KEY: &str = "federation.minimum_protocol_version";
const BUNDLE_DOMAIN: &[u8] = b"godmesh.federation.config.bundle.v1";
const VOTE_DOMAIN: &[u8] = b"godmesh.federation.config.vote.v1";

/// Votante provisionado por uma fonte confiável do chamador.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedVoter {
    pub node_id: String,
    pub public_key: String,
}

/// Política local do perfil. O limite de bytes é inclusivo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundlePolicy {
    pub cluster: String,
    pub epoch: u64,
    pub max_frame_bytes: NonZeroUsize,
    pub voters: Vec<PinnedVoter>,
}

impl BundlePolicy {
    /// Valida identidade e quórum estático do perfil, sem depender de um frame.
    pub fn validate(&self) -> Result<(), BundleError> {
        if valid_policy(self) {
            Ok(())
        } else {
            Err(BundleError::InvalidPolicy)
        }
    }
}

/// Componentes de versão de software comparados em ordem de major, minor e patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SoftwareVersion([u64; 3]);

impl SoftwareVersion {
    /// Cada componente deve caber na faixa interoperável do perfil.
    #[must_use]
    pub fn new(major: u64, minor: u64, patch: u64) -> Option<Self> {
        let parts = [major, minor, patch];
        parts
            .iter()
            .all(|&part| part <= MAX_INTEGER)
            .then_some(Self(parts))
    }

    #[must_use]
    pub fn components(self) -> [u64; 3] {
        self.0
    }

    fn parse(text: &str) -> Option<Self> {
        let mut fields = text.split('.');
        let mut parts = [0; 3];
        for part in &mut parts {
            let field = fields.next()?;
            if (field.len() > 1 && field.starts_with('0'))
                || !field.bytes().all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            *part = field.parse().ok()?;
        }
        if fields.next().is_some() {
            return None;
        }
        Self::new(parts[0], parts[1], parts[2])
    }
}

/// Chave completa de uma série de snapshots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleKey {
    pub cluster: String,
    pub namespace: String,
    pub scope_id: String,
    pub epoch: u64,
}

/// Campos cobertos pelo hash do perfil 1. O valor é o piso de protocolo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleBody {
    pub key: BundleKey,
    pub version: u64,
    pub applies_at: i64,
    pub expires_at: i64,
    pub required_signatures: u64,
    pub value: u64,
    pub effective_after: i64,
    pub min_version: SoftwareVersion,
}

impl BundleBody {
    /// Codifica os campos segundo o TLV do perfil 1.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = BUNDLE_DOMAIN.to_vec();
        push_field(&mut bytes, "profile", &PROFILE.to_be_bytes());
        push_field(&mut bytes, "cluster", self.key.cluster.as_bytes());
        push_field(&mut bytes, "namespace", self.key.namespace.as_bytes());
        push_field(&mut bytes, "scope_id", self.key.scope_id.as_bytes());
        push_field(&mut bytes, "epoch", &self.key.epoch.to_be_bytes());
        push_field(&mut bytes, "version", &self.version.to_be_bytes());
        push_field(&mut bytes, "applies_at", &self.applies_at.to_be_bytes());
        push_field(&mut bytes, "expires_at", &self.expires_at.to_be_bytes());
        push_field(
            &mut bytes,
            "required_signatures",
            &self.required_signatures.to_be_bytes(),
        );
        push_field(&mut bytes, "entry_count", &1_u64.to_be_bytes());
        push_field(&mut bytes, "key", ENTRY_KEY.as_bytes());
        push_field(&mut bytes, "value", &self.value.to_be_bytes());
        push_field(
            &mut bytes,
            "effective_after",
            &self.effective_after.to_be_bytes(),
        );
        let mut version = [0; 24];
        let (fields, _) = version.as_chunks_mut::<8>();
        for (field, component) in fields.iter_mut().zip(self.min_version.components()) {
            *field = component.to_be_bytes();
        }
        push_field(&mut bytes, "min_version", &version);
        bytes
    }

    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.canonical_bytes()).into()
    }
}

/// Preimagem de voto, com domínio próprio e hash em bytes brutos.
#[must_use]
pub fn vote_bytes(hash: &[u8; 32]) -> Vec<u8> {
    let mut bytes = VOTE_DOMAIN.to_vec();
    push_field(&mut bytes, "hash", hash);
    bytes
}

/// Maior versão aceita da série, obtida de fonte confiável pelo chamador.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryRecord {
    pub key: BundleKey,
    pub version: u64,
    pub hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryStatus {
    HistoryKeyMismatch,
    Rollback,
    Replay,
    Conflict,
    NewVersion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleStatus {
    Expired,
    IncompatibleSoftware,
    Pending,
    Eligible,
}

/// Corpo autenticado por todos os votantes da política informada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedBundle {
    body: BundleBody,
    hash: [u8; 32],
}

impl AuthenticatedBundle {
    #[must_use]
    pub fn body(&self) -> &BundleBody {
        &self.body
    }

    #[must_use]
    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    /// Classifica a versão em relação ao registro fornecido, sem consultar o relógio.
    #[must_use]
    pub fn compare_history(&self, history: Option<&HistoryRecord>) -> HistoryStatus {
        let Some(history) = history else {
            return HistoryStatus::NewVersion;
        };
        if history.key != self.body.key {
            return HistoryStatus::HistoryKeyMismatch;
        }
        match self.body.version.cmp(&history.version) {
            Ordering::Less => HistoryStatus::Rollback,
            Ordering::Equal if history.hash == self.hash => HistoryStatus::Replay,
            Ordering::Equal => HistoryStatus::Conflict,
            Ordering::Greater => HistoryStatus::NewVersion,
        }
    }

    /// Avalia a janela e a versão de software para o instante UTC fornecido.
    #[must_use]
    pub fn evaluate(&self, now: i64, software: SoftwareVersion) -> BundleStatus {
        if now >= self.body.expires_at {
            BundleStatus::Expired
        } else if software < self.body.min_version {
            BundleStatus::IncompatibleSoftware
        } else if now < self.body.applies_at.max(self.body.effective_after) {
            BundleStatus::Pending
        } else {
            BundleStatus::Eligible
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleError {
    InvalidPolicy,
    FrameTooLarge,
    MalformedFrame,
    UnsupportedProfile,
    InvalidField(&'static str),
    ClusterMismatch,
    EpochMismatch,
    HashMismatch,
    ThresholdMismatch,
    DuplicateVote,
    UnknownVoter,
    InvalidSignature,
    InsufficientQuorum,
}

impl fmt::Display for BundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for BundleError {}

// O JSON já validou a gramática de números. Preservar o lexema permite
// distinguir -0, fração, expoente e overflow antes de qualquer conversão.
struct Integer<'a>(&'a str);

impl<'de: 'a, 'a> Deserialize<'de> for Integer<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <&RawValue>::deserialize(deserializer)?.get();
        let digits = raw.strip_prefix('-').unwrap_or(raw);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(D::Error::custom("esperado um inteiro JSON"));
        }
        Ok(Self(raw))
    }
}

impl Integer<'_> {
    fn unsigned(&self, minimum: u64, field: &'static str) -> Result<u64, BundleError> {
        self.0
            .parse()
            .ok()
            .filter(|value| (minimum..=MAX_INTEGER).contains(value))
            .ok_or(BundleError::InvalidField(field))
    }

    fn timestamp(&self, field: &'static str) -> Result<i64, BundleError> {
        self.0.parse().map_err(|_| BundleError::InvalidField(field))
    }
}

// Structs derivadas pelo Serde aceitam arrays posicionais. Esta fronteira
// exige objetos sem materializá-los em Value, para preservar lexemas e duplicatas.
fn object<'de, T: Deserialize<'de>>(json: &'de str) -> Result<T, BundleError> {
    let json = json.trim_start_matches([' ', '\t', '\r', '\n']);
    if !json.starts_with('{') {
        return Err(BundleError::MalformedFrame);
    }
    serde_json::from_str(json).map_err(|_| BundleError::MalformedFrame)
}

fn objects<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    Vec::<&RawValue>::deserialize(deserializer)?
        .into_iter()
        .map(|raw| object(raw.get()).map_err(D::Error::custom))
        .collect()
}

#[derive(Deserialize)]
struct ProfileProbe<'a> {
    #[serde(borrow)]
    profile: Integer<'a>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBundle<'a> {
    #[serde(rename = "profile", borrow)]
    _profile: Integer<'a>,
    cluster: String,
    namespace: String,
    scope_id: String,
    #[serde(borrow)]
    epoch: Integer<'a>,
    #[serde(borrow)]
    version: Integer<'a>,
    #[serde(borrow)]
    applies_at: Integer<'a>,
    #[serde(borrow)]
    expires_at: Integer<'a>,
    #[serde(borrow)]
    required_signatures: Integer<'a>,
    #[serde(borrow, deserialize_with = "objects")]
    entries: Vec<WireEntry<'a>>,
    hash: String,
    #[serde(deserialize_with = "objects")]
    signatures: Vec<WireVote>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEntry<'a> {
    key: String,
    #[serde(borrow)]
    value: Integer<'a>,
    #[serde(borrow)]
    effective_after: Integer<'a>,
    min_version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireVote {
    signer_node_id: String,
    signature: String,
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_cluster(cluster: &str) -> bool {
    cluster.len() <= MAX_FIELD_LEN && !cluster.trim().is_empty()
}

fn valid_policy(policy: &BundlePolicy) -> bool {
    let mut ids = HashSet::new();
    let mut keys = HashSet::new();
    valid_cluster(&policy.cluster)
        && policy.epoch <= MAX_INTEGER
        && matches!(policy.voters.len(), 3 | 5)
        && policy.voters.iter().all(|voter| {
            lower_hex(&voter.public_key, 64)
                && identity::verify_node_id(&voter.node_id, &voter.public_key)
                && ids.insert(&voter.node_id)
                && keys.insert(&voter.public_key)
        })
}

impl WireBundle<'_> {
    // Toda a desserialização estrita precede os limites semânticos abaixo.
    fn validate_body(&self) -> Result<BundleBody, BundleError> {
        use BundleError::InvalidField;

        if !valid_cluster(&self.cluster) {
            return Err(InvalidField("cluster"));
        }
        if self.namespace != NAMESPACE {
            return Err(InvalidField("namespace"));
        }
        if !self.scope_id.is_empty() {
            return Err(InvalidField("scope_id"));
        }
        let epoch = self.epoch.unsigned(0, "epoch")?;
        let version = self.version.unsigned(1, "version")?;
        let applies_at = self.applies_at.timestamp("applies_at")?;
        let expires_at = self.expires_at.timestamp("expires_at")?;
        if applies_at >= expires_at {
            return Err(InvalidField("expires_at"));
        }
        let required_signatures = self
            .required_signatures
            .unsigned(0, "required_signatures")?;
        if !matches!(required_signatures, 3 | 5) {
            return Err(InvalidField("required_signatures"));
        }
        let [entry] = self.entries.as_slice() else {
            return Err(InvalidField("entries"));
        };
        if entry.key != ENTRY_KEY {
            return Err(InvalidField("key"));
        }
        let value = entry.value.unsigned(1, "value")?;
        let effective_after = entry.effective_after.timestamp("effective_after")?;
        if effective_after >= expires_at {
            return Err(InvalidField("effective_after"));
        }
        let min_version =
            SoftwareVersion::parse(&entry.min_version).ok_or(InvalidField("min_version"))?;
        if !lower_hex(&self.hash, 64) {
            return Err(InvalidField("hash"));
        }
        if self.signatures.len() > 5 {
            return Err(InvalidField("signatures"));
        }
        for vote in &self.signatures {
            if !identity::is_node_id(&vote.signer_node_id) {
                return Err(InvalidField("signer_node_id"));
            }
            if !lower_hex(&vote.signature, 128) {
                return Err(InvalidField("signature"));
            }
        }

        Ok(BundleBody {
            key: BundleKey {
                cluster: self.cluster.clone(),
                namespace: self.namespace.clone(),
                scope_id: self.scope_id.clone(),
                epoch,
            },
            version,
            applies_at,
            expires_at,
            required_signatures,
            value,
            effective_after,
            min_version,
        })
    }
}

fn verify_votes(
    mut votes: Vec<WireVote>,
    policy: &BundlePolicy,
    hash: &[u8; 32],
) -> Result<(), BundleError> {
    votes.sort_unstable_by(|a, b| a.signer_node_id.cmp(&b.signer_node_id));
    if votes
        .windows(2)
        .any(|pair| pair[0].signer_node_id == pair[1].signer_node_id)
    {
        return Err(BundleError::DuplicateVote);
    }
    let bytes = vote_bytes(hash);
    for vote in &votes {
        let voter = policy
            .voters
            .iter()
            .find(|voter| voter.node_id == vote.signer_node_id)
            .ok_or(BundleError::UnknownVoter)?;
        identity::verify_signature(&voter.public_key, &bytes, &vote.signature)
            .map_err(|_| BundleError::InvalidSignature)?;
    }
    if votes.len() < policy.voters.len() {
        return Err(BundleError::InsufficientQuorum);
    }
    Ok(())
}

/// Autentica um frame M3-A para a política explícita do chamador.
///
/// # Errors
/// Retorna a primeira falha segundo a precedência da SPEC M3-A.
pub fn authenticate_bundle(
    frame: &[u8],
    policy: &BundlePolicy,
) -> Result<AuthenticatedBundle, BundleError> {
    use BundleError::*;

    policy.validate()?;
    if frame.len() > policy.max_frame_bytes.get() {
        return Err(FrameTooLarge);
    }
    let json = std::str::from_utf8(frame).map_err(|_| MalformedFrame)?;
    let probe: ProfileProbe<'_> = object(json)?;
    let profile: u64 = probe.profile.0.parse().map_err(|_| MalformedFrame)?;
    if profile != PROFILE {
        return Err(UnsupportedProfile);
    }

    let wire: WireBundle<'_> = object(json)?;
    let body = wire.validate_body()?;
    if body.key.cluster != policy.cluster {
        return Err(ClusterMismatch);
    }
    if body.key.epoch != policy.epoch {
        return Err(EpochMismatch);
    }
    let hash = body.digest();
    if hex::encode(hash) != wire.hash {
        return Err(HashMismatch);
    }
    if body.required_signatures != policy.voters.len() as u64 {
        return Err(ThresholdMismatch);
    }
    verify_votes(wire.signatures, policy, &hash)?;
    Ok(AuthenticatedBundle { body, hash })
}
