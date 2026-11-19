//! Evidência autenticada do restore planejado M4-B2-C. Todas as entradas são explícitas.

mod commitments;

use std::{fmt, num::NonZeroUsize};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

use crate::{
    NodeIdentity, identity,
    manifest::{MAX_FIELD_LEN, push_field},
};

pub use commitments::{
    ControlState, CopyAcl, CopyPolicy, Fence, Reconciliation, RevocationReference, proofs_digest,
};

const MAX_INT: u64 = i64::MAX as u64;
const HIGH_WATER_DOMAIN: &[u8] = b"godmesh.federation.restore.high-water.v1";
const PROMOTION_DOMAIN: &[u8] = b"godmesh.federation.restore.promotion.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreError {
    FrameTooLarge,
    MalformedFrame,
    InvalidField(&'static str),
    InvalidGeneration,
    InvalidCut,
    InvalidSuspension,
    InvalidIntent,
    InvalidFence,
    InvalidWindow,
    OutsideWindow,
    InvalidSignature,
    BindingMismatch,
    BlockedIntent,
    ProofCountMismatch,
    InvalidProofLength,
    InvalidRevocationSet,
}

impl fmt::Display for RestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RestoreError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreKey {
    pub fixture_id: String,
    pub cluster: String,
    pub local_node: String,
    pub origin_node: String,
    pub tenant_id: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    AtCut,
    BeforeCut,
}

impl BackupKind {
    fn tag(self) -> &'static [u8] {
        match self {
            Self::AtCut => b"at_cut",
            Self::BeforeCut => b"before_cut",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
#[repr(u8)]
pub enum IntentState {
    Absent = 0,
    Pending = 1,
    Indeterminate = 2,
}

impl From<IntentState> for u8 {
    fn from(state: IntentState) -> Self {
        state as Self
    }
}

impl TryFrom<u8> for IntentState {
    type Error = RestoreError;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0 => Ok(Self::Absent),
            1 => Ok(Self::Pending),
            2 => Ok(Self::Indeterminate),
            _ => Err(RestoreError::InvalidIntent),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuthorityKind {
    Genesis = 0,
    Leaf = 1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreHighWaterV1 {
    pub version: u64,
    #[serde(deserialize_with = "key_object")]
    pub key: RestoreKey,
    pub backup_db_id: String,
    pub target_db_id: String,
    pub backup_generation: u64,
    pub fenced_generation: u64,
    pub target_generation: u64,
    pub backup_kind: BackupKind,
    pub backup_bytes: u64,
    #[serde(with = "hex_wire")]
    pub backup_sha256: [u8; 32],
    pub backup_prefix: u64,
    pub cut_prefix: u64,
    pub backup_proof_count: u64,
    #[serde(with = "hex_wire")]
    pub backup_proofs_sha256: [u8; 32],
    pub gap_proof_count: u64,
    #[serde(with = "hex_wire")]
    pub gap_proofs_sha256: [u8; 32],
    #[serde(with = "hex_wire")]
    pub control_state_sha256: [u8; 32],
    pub blocked_next_seq: u64,
    pub intent_state: IntentState,
    pub attempt_id: u64,
    #[serde(with = "hex_wire")]
    pub fence_sha256: [u8; 32],
    pub cut_at: i64,
    #[serde(with = "hex_wire")]
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestorePromotionV1 {
    pub version: u64,
    #[serde(deserialize_with = "key_object")]
    pub key: RestoreKey,
    pub target_db_id: String,
    pub fenced_generation: u64,
    pub target_generation: u64,
    #[serde(with = "hex_wire")]
    pub high_water_sha256: [u8; 32],
    #[serde(with = "hex_wire")]
    pub reconciliation_sha256: [u8; 32],
    #[serde(with = "hex_wire")]
    pub fence_sha256: [u8; 32],
    pub not_before: i64,
    pub expires_at: i64,
    #[serde(with = "hex_wire")]
    pub signature: [u8; 64],
}

fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn starts_with_object(json: &str) -> bool {
    json.trim_start_matches([' ', '\t', '\n', '\r'])
        .starts_with('{')
}

fn key_object<'de, D: Deserializer<'de>>(decoder: D) -> Result<RestoreKey, D::Error> {
    let raw = <&RawValue>::deserialize(decoder)?;
    if !starts_with_object(raw.get()) {
        return Err(D::Error::custom("key deve ser um objeto JSON"));
    }
    serde_json::from_str(raw.get()).map_err(D::Error::custom)
}

mod hex_wire {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    pub(super) fn serialize<S: Serializer, const N: usize>(
        bytes: &[u8; N],
        encoder: S,
    ) -> Result<S::Ok, S::Error> {
        encoder.serialize_str(&hex::encode(bytes))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        decoder: D,
    ) -> Result<[u8; N], D::Error> {
        let encoded = String::deserialize(decoder)?;
        if !super::lower_hex(&encoded, N * 2) {
            return Err(D::Error::custom("hex minúsculo no tamanho exato"));
        }
        let mut bytes = [0; N];
        hex::decode_to_slice(&encoded, &mut bytes).map_err(D::Error::custom)?;
        Ok(bytes)
    }
}

fn decode<T: for<'de> Deserialize<'de>>(
    json: &[u8],
    max_bytes: NonZeroUsize,
) -> Result<T, RestoreError> {
    if json.len() > max_bytes.get() {
        return Err(RestoreError::FrameTooLarge);
    }
    let json = std::str::from_utf8(json).map_err(|_| RestoreError::MalformedFrame)?;
    if !starts_with_object(json) {
        return Err(RestoreError::MalformedFrame);
    }
    serde_json::from_str(json).map_err(|_| RestoreError::MalformedFrame)
}

fn valid(condition: bool, name: &'static str) -> Result<(), RestoreError> {
    if !condition {
        return Err(RestoreError::InvalidField(name));
    }
    Ok(())
}

fn positive(value: u64, name: &'static str) -> Result<(), RestoreError> {
    valid((1..=MAX_INT).contains(&value), name)
}

fn range(value: u64, name: &'static str) -> Result<(), RestoreError> {
    valid(value <= MAX_INT, name)
}

fn check_generations(fenced: u64, target: u64) -> Result<(), RestoreError> {
    if fenced == 0 || target > MAX_INT || fenced.checked_add(1) != Some(target) {
        return Err(RestoreError::InvalidGeneration);
    }
    Ok(())
}

fn check_suspension(prefix: u64, blocked_next_seq: u64) -> Result<(), RestoreError> {
    if blocked_next_seq != 0
        && (blocked_next_seq > MAX_INT || prefix.checked_add(1) != Some(blocked_next_seq))
    {
        return Err(RestoreError::InvalidSuspension);
    }
    Ok(())
}

fn check_intent(state: IntentState, attempt_id: u64) -> Result<(), RestoreError> {
    let valid = match state {
        IntentState::Pending => (1..=MAX_INT).contains(&attempt_id),
        IntentState::Absent | IntentState::Indeterminate => attempt_id == 0,
    };
    if !valid {
        return Err(RestoreError::InvalidIntent);
    }
    Ok(())
}

struct Tlv(Vec<u8>);

impl Tlv {
    fn new(domain: &[u8]) -> Self {
        Self(domain.to_vec())
    }

    fn bytes(&mut self, tag: &str, value: &[u8]) {
        push_field(&mut self.0, tag, value);
    }

    fn text(&mut self, tag: &'static str, value: &str) -> Result<(), RestoreError> {
        valid(
            !value.trim().is_empty() && value.len() <= MAX_FIELD_LEN,
            tag,
        )?;
        self.bytes(tag, value.as_bytes());
        Ok(())
    }

    fn node(&mut self, tag: &'static str, value: &str) -> Result<(), RestoreError> {
        valid(identity::is_node_id(value), tag)?;
        self.bytes(tag, value.as_bytes());
        Ok(())
    }

    fn u64(&mut self, tag: &str, value: u64) {
        self.bytes(tag, &value.to_be_bytes());
    }

    fn i64(&mut self, tag: &str, value: i64) {
        self.bytes(tag, &value.to_be_bytes());
    }

    fn u32(&mut self, tag: &str, value: u32) {
        self.bytes(tag, &value.to_be_bytes());
    }

    fn byte(&mut self, tag: &str, value: u8) {
        self.bytes(tag, &[value]);
    }

    fn key(&mut self, key: &RestoreKey) -> Result<(), RestoreError> {
        valid(lower_hex(&key.fixture_id, 32), "fixture_id")?;
        self.bytes("fixture_id", key.fixture_id.as_bytes());
        self.text("cluster", &key.cluster)?;
        self.node("local_node", &key.local_node)?;
        self.node("origin_node", &key.origin_node)?;
        valid(key.local_node != key.origin_node, "origin_node")?;
        self.text("tenant_id", &key.tenant_id)?;
        self.text("stream_id", &key.stream_id)?;
        valid(key.partition == 0, "partition")?;
        self.u32("partition", key.partition);
        positive(key.authority_epoch, "authority_epoch")?;
        self.u64("authority_epoch", key.authority_epoch);
        Ok(())
    }

    fn intent(&mut self, state: IntentState, attempt_id: u64) {
        self.byte("intent_state", state.into());
        self.u64("attempt_id", attempt_id);
    }
}

fn sha256(bytes: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

impl RestoreHighWaterV1 {
    /// Preimagem assinada da alta marca.
    ///
    /// # Errors
    /// Recusa campos ou invariantes fora da v1.4.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreError> {
        use RestoreError::*;

        valid(self.version == 1, "version")?;
        let mut tlv = Tlv::new(HIGH_WATER_DOMAIN);
        tlv.u64("version", self.version);
        tlv.key(&self.key)?;
        tlv.text("backup_db_id", &self.backup_db_id)?;
        tlv.text("target_db_id", &self.target_db_id)?;
        if self.backup_generation == 0 || self.backup_generation > self.fenced_generation {
            return Err(InvalidGeneration);
        }
        check_generations(self.fenced_generation, self.target_generation)?;
        tlv.u64("backup_generation", self.backup_generation);
        tlv.u64("fenced_generation", self.fenced_generation);
        tlv.u64("target_generation", self.target_generation);
        tlv.bytes("backup_kind", self.backup_kind.tag());
        range(self.backup_bytes, "backup_bytes")?;
        tlv.u64("backup_bytes", self.backup_bytes);
        tlv.bytes("backup_sha256", &self.backup_sha256);
        range(self.backup_prefix, "backup_prefix")?;
        range(self.cut_prefix, "cut_prefix")?;
        let gap = self
            .cut_prefix
            .checked_sub(self.backup_prefix)
            .ok_or(InvalidCut)?;
        if (gap == 0) != (self.backup_kind == BackupKind::AtCut)
            || self.backup_proof_count != self.backup_prefix
            || self.gap_proof_count != gap
        {
            return Err(InvalidCut);
        }
        let empty = proofs_digest(0, [])?;
        if (self.backup_proof_count == 0 && self.backup_proofs_sha256 != empty)
            || (self.gap_proof_count == 0 && self.gap_proofs_sha256 != empty)
        {
            return Err(InvalidCut);
        }
        tlv.u64("backup_prefix", self.backup_prefix);
        tlv.u64("cut_prefix", self.cut_prefix);
        tlv.u64("backup_proof_count", self.backup_proof_count);
        tlv.bytes("backup_proofs_sha256", &self.backup_proofs_sha256);
        tlv.u64("gap_proof_count", self.gap_proof_count);
        tlv.bytes("gap_proofs_sha256", &self.gap_proofs_sha256);
        tlv.bytes("control_state_sha256", &self.control_state_sha256);
        check_suspension(self.cut_prefix, self.blocked_next_seq)?;
        tlv.u64("blocked_next_seq", self.blocked_next_seq);
        check_intent(self.intent_state, self.attempt_id)?;
        tlv.intent(self.intent_state, self.attempt_id);
        tlv.bytes("fence_sha256", &self.fence_sha256);
        tlv.i64("cut_at", self.cut_at);
        Ok(tlv.0)
    }

    /// SHA-256 do TLV assinado.
    ///
    /// # Errors
    /// Propaga falhas de validação da preimagem.
    pub fn digest(&self) -> Result<[u8; 32], RestoreError> {
        self.canonical_bytes().map(sha256)
    }

    /// Assina com a identidade explícita do harness.
    ///
    /// # Errors
    /// Propaga falhas de validação da preimagem.
    pub fn sign(&mut self, signer: &NodeIdentity) -> Result<(), RestoreError> {
        self.signature = signer.sign(&self.canonical_bytes()?).to_bytes();
        Ok(())
    }
}

impl RestorePromotionV1 {
    /// Preimagem assinada da autorização.
    ///
    /// # Errors
    /// Recusa campos ou invariantes fora da v1.4.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreError> {
        valid(self.version == 1, "version")?;
        let mut tlv = Tlv::new(PROMOTION_DOMAIN);
        tlv.u64("version", self.version);
        tlv.key(&self.key)?;
        tlv.text("target_db_id", &self.target_db_id)?;
        check_generations(self.fenced_generation, self.target_generation)?;
        tlv.u64("fenced_generation", self.fenced_generation);
        tlv.u64("target_generation", self.target_generation);
        tlv.bytes("high_water_sha256", &self.high_water_sha256);
        tlv.bytes("reconciliation_sha256", &self.reconciliation_sha256);
        tlv.bytes("fence_sha256", &self.fence_sha256);
        if self.not_before >= self.expires_at {
            return Err(RestoreError::InvalidWindow);
        }
        tlv.i64("not_before", self.not_before);
        tlv.i64("expires_at", self.expires_at);
        Ok(tlv.0)
    }

    /// SHA-256 do TLV assinado.
    ///
    /// # Errors
    /// Propaga falhas de validação da preimagem.
    pub fn digest(&self) -> Result<[u8; 32], RestoreError> {
        self.canonical_bytes().map(sha256)
    }

    /// Assina com a identidade explícita do harness.
    ///
    /// # Errors
    /// Propaga falhas de validação da preimagem.
    pub fn sign(&mut self, signer: &NodeIdentity) -> Result<(), RestoreError> {
        self.signature = signer.sign(&self.canonical_bytes()?).to_bytes();
        Ok(())
    }
}

/// Vínculos da fonte externa, obtidos fora do arquivo restaurado.
#[derive(Debug, Clone)]
pub struct ExpectedHighWater<'a> {
    pub key: &'a RestoreKey,
    pub target_db_id: &'a str,
    pub fenced_generation: u64,
    pub target_generation: u64,
    pub high_water_sha256: [u8; 32],
    pub fence_sha256: [u8; 32],
    pub harness_public_key: &'a str,
}

/// Vínculos do alvo reconciliado, conferidos pelo chamador sob os guardas locais.
#[derive(Debug, Clone)]
pub struct ExpectedPromotion<'a> {
    pub high_water: &'a VerifiedHighWater,
    pub reconciliation: &'a Reconciliation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedHighWater {
    high_water: RestoreHighWaterV1,
    digest: [u8; 32],
    harness_public_key: String,
}

impl VerifiedHighWater {
    #[must_use]
    pub fn high_water(&self) -> &RestoreHighWaterV1 {
        &self.high_water
    }

    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    /// Confere o artefato de backup pelos bytes completos e o comprimento.
    ///
    /// # Errors
    /// Recusa divergência de tamanho ou de hash.
    pub fn verify_backup(&self, bytes: &[u8]) -> Result<(), RestoreError> {
        if u64::try_from(bytes.len()).ok() != Some(self.high_water.backup_bytes)
            || sha256(bytes) != self.high_water.backup_sha256
        {
            return Err(RestoreError::BindingMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPromotion {
    promotion: RestorePromotionV1,
    digest: [u8; 32],
}

impl VerifiedPromotion {
    #[must_use]
    pub fn promotion(&self) -> &RestorePromotionV1 {
        &self.promotion
    }

    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

fn authenticate(body: &[u8], signature: &[u8; 64], public_key: &str) -> Result<(), RestoreError> {
    identity::verify_signature(public_key, body, &hex::encode(signature))
        .map_err(|_| RestoreError::InvalidSignature)
}

/// Autentica a alta marca e exige correspondência com a referência externa corrente.
///
/// # Errors
/// Recusa JSON, campos, assinatura ou vínculo externo inválidos.
pub fn verify_high_water(
    json: &[u8],
    max_bytes: NonZeroUsize,
    expected: &ExpectedHighWater<'_>,
) -> Result<VerifiedHighWater, RestoreError> {
    let high_water: RestoreHighWaterV1 = decode(json, max_bytes)?;
    let body = high_water.canonical_bytes()?;
    authenticate(&body, &high_water.signature, expected.harness_public_key)?;
    let digest = sha256(body);
    if &high_water.key != expected.key
        || high_water.target_db_id != expected.target_db_id
        || high_water.fenced_generation != expected.fenced_generation
        || high_water.target_generation != expected.target_generation
        || high_water.fence_sha256 != expected.fence_sha256
        || digest != expected.high_water_sha256
    {
        return Err(RestoreError::BindingMismatch);
    }
    Ok(VerifiedHighWater {
        high_water,
        digest,
        harness_public_key: expected.harness_public_key.into(),
    })
}

/// Autentica a promoção para os vínculos e o instante indicados, antes do CAS.
///
/// # Errors
/// Recusa artefato inválido, vínculo divergente, intenção bloqueante ou janela vencida.
pub fn verify_promotion(
    json: &[u8],
    max_bytes: NonZeroUsize,
    expected: &ExpectedPromotion<'_>,
    now: i64,
) -> Result<VerifiedPromotion, RestoreError> {
    use RestoreError::*;

    let promotion: RestorePromotionV1 = decode(json, max_bytes)?;
    let body = promotion.canonical_bytes()?;
    let high = &expected.high_water.high_water;
    authenticate(
        &body,
        &promotion.signature,
        &expected.high_water.harness_public_key,
    )?;
    let rec = expected.reconciliation;
    if high.intent_state != IntentState::Absent || rec.intent_state != IntentState::Absent {
        return Err(BlockedIntent);
    }
    let rec_digest = rec.digest()?;
    if promotion.key != high.key
        || promotion.target_db_id != high.target_db_id
        || promotion.fenced_generation != high.fenced_generation
        || promotion.target_generation != high.target_generation
        || promotion.high_water_sha256 != expected.high_water.digest
        || promotion.fence_sha256 != high.fence_sha256
        || promotion.reconciliation_sha256 != rec_digest
        || rec.key != high.key
        || rec.target_generation != high.target_generation
        || rec.high_water_sha256 != expected.high_water.digest
        || rec.final_prefix != high.cut_prefix
        || rec.blocked_next_seq != high.blocked_next_seq
    {
        return Err(BindingMismatch);
    }
    if now < promotion.not_before || now >= promotion.expires_at {
        return Err(OutsideWindow);
    }
    Ok(VerifiedPromotion {
        promotion,
        digest: sha256(body),
    })
}
