use sha2::{Digest, Sha256};

use super::{
    AuthorityKind, IntentState, RestoreError, RestoreKey, Tlv, check_intent, check_suspension,
    positive, range, sha256, valid,
};

const PROOFS_DOMAIN: &[u8] = b"godmesh.federation.restore.proofs.v1";

/// Compromete a contagem declarada e os frames originais na ordem recebida.
///
/// # Errors
/// Recusa contagem fora da faixa, contagem divergente ou tamanho inválido.
pub fn proofs_digest<'a>(
    count: u64,
    frames: impl IntoIterator<Item = &'a [u8]>,
) -> Result<[u8; 32], RestoreError> {
    range(count, "proof_count")?;
    let mut digest = Sha256::new();
    digest.update(PROOFS_DOMAIN);
    digest.update(count.to_be_bytes());
    let mut remaining = count;
    for frame in frames {
        remaining = remaining
            .checked_sub(1)
            .ok_or(RestoreError::ProofCountMismatch)?;
        let len = u32::try_from(frame.len()).map_err(|_| RestoreError::InvalidProofLength)?;
        if len == 0 {
            return Err(RestoreError::InvalidProofLength);
        }
        digest.update(len.to_be_bytes());
        digest.update(frame);
    }
    if remaining != 0 {
        return Err(RestoreError::ProofCountMismatch);
    }
    Ok(digest.finalize().into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyPolicy {
    pub policy_id: String,
    pub policy_version: u64,
    pub tenant_id: String,
    pub retention_class: String,
    pub class_version: u64,
    pub jurisdiction: String,
    pub copy_node: String,
    pub copy_kind: String,
    pub purpose: String,
    pub trigger_event: String,
    pub duration_seconds: u64,
    pub allow_discard: bool,
    pub hold_mode: String,
    pub active: bool,
}

impl CopyPolicy {
    /// Preimagem da linha de política obtida no banco correspondente.
    ///
    /// # Errors
    /// Recusa campos inválidos ou fora da faixa persistível.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreError> {
        let mut tlv = Tlv::new(b"godmesh.federation.restore.copy-policy.v1");
        tlv.text("policy_id", &self.policy_id)?;
        positive(self.policy_version, "policy_version")?;
        tlv.u64("policy_version", self.policy_version);
        tlv.text("tenant_id", &self.tenant_id)?;
        tlv.text("retention_class", &self.retention_class)?;
        positive(self.class_version, "class_version")?;
        tlv.u64("class_version", self.class_version);
        tlv.text("jurisdiction", &self.jurisdiction)?;
        tlv.node("copy_node", &self.copy_node)?;
        tlv.text("copy_kind", &self.copy_kind)?;
        tlv.text("purpose", &self.purpose)?;
        tlv.text("trigger_event", &self.trigger_event)?;
        positive(self.duration_seconds, "duration_seconds")?;
        tlv.u64("duration_seconds", self.duration_seconds);
        tlv.byte("allow_discard", u8::from(self.allow_discard));
        tlv.text("hold_mode", &self.hold_mode)?;
        tlv.byte("active", u8::from(self.active));
        Ok(tlv.0)
    }

    /// # Errors
    /// Propaga falhas da preimagem.
    pub fn digest(&self) -> Result<[u8; 32], RestoreError> {
        self.canonical_bytes().map(sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyAcl {
    pub origin_node: String,
    pub tenant_id: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    pub destination: String,
    pub direction: String,
    pub retention_class: String,
    pub class_version: u64,
    pub jurisdiction: String,
    pub copy_kind: String,
    pub policy_id: String,
    pub policy_version: u64,
    pub not_before: i64,
    pub expires_at: i64,
    pub revoked: bool,
}

impl CopyAcl {
    /// Preimagem da linha de ACL obtida no banco correspondente.
    ///
    /// # Errors
    /// Recusa campos inválidos ou janela invertida.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreError> {
        let mut tlv = Tlv::new(b"godmesh.federation.restore.copy-acl.v1");
        tlv.node("origin_node", &self.origin_node)?;
        tlv.text("tenant_id", &self.tenant_id)?;
        tlv.text("stream_id", &self.stream_id)?;
        valid(self.partition == 0, "partition")?;
        tlv.u32("partition", self.partition);
        positive(self.authority_epoch, "authority_epoch")?;
        tlv.u64("authority_epoch", self.authority_epoch);
        tlv.node("destination", &self.destination)?;
        valid(self.origin_node != self.destination, "destination")?;
        tlv.text("direction", &self.direction)?;
        tlv.text("retention_class", &self.retention_class)?;
        positive(self.class_version, "class_version")?;
        tlv.u64("class_version", self.class_version);
        tlv.text("jurisdiction", &self.jurisdiction)?;
        tlv.text("copy_kind", &self.copy_kind)?;
        tlv.text("policy_id", &self.policy_id)?;
        positive(self.policy_version, "policy_version")?;
        tlv.u64("policy_version", self.policy_version);
        if self.not_before >= self.expires_at {
            return Err(RestoreError::InvalidWindow);
        }
        tlv.i64("not_before", self.not_before);
        tlv.i64("expires_at", self.expires_at);
        tlv.byte("revoked", u8::from(self.revoked));
        Ok(tlv.0)
    }

    /// # Errors
    /// Propaga falhas da preimagem.
    pub fn digest(&self) -> Result<[u8; 32], RestoreError> {
        self.canonical_bytes().map(sha256)
    }
}

/// Referência obtida após validar declaração, cadeia alvo e recibo M1C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationReference {
    pub cluster: String,
    pub root_node_id: String,
    pub target_digest: [u8; 32],
    pub revocation_digest: [u8; 32],
    pub accepted_at: i64,
    pub receipt_signature: [u8; 64],
}

impl RevocationReference {
    /// # Errors
    /// Recusa cluster ou ID de raiz inválido.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreError> {
        let mut tlv = Tlv::new(b"godmesh.federation.restore.revocation.v1");
        tlv.text("cluster", &self.cluster)?;
        tlv.node("root_node_id", &self.root_node_id)?;
        tlv.bytes("target_digest", &self.target_digest);
        tlv.bytes("revocation_digest", &self.revocation_digest);
        tlv.i64("accepted_at", self.accepted_at);
        tlv.bytes("receipt_signature", &self.receipt_signature);
        Ok(tlv.0)
    }

    /// # Errors
    /// Propaga falhas da preimagem.
    pub fn digest(&self) -> Result<[u8; 32], RestoreError> {
        self.canonical_bytes().map(sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlState {
    pub key: RestoreKey,
    pub generation: u64,
    pub root_node_id: String,
    pub authority_kind: AuthorityKind,
    pub authority_ref: [u8; 32],
    /// Conjunto completo do banco de A; vazio significa ausência comprovada.
    pub origin_revocations: Vec<[u8; 32]>,
    /// Conjunto completo do banco de B; vazio significa ausência comprovada.
    pub destination_revocations: Vec<[u8; 32]>,
    pub m3_epoch: u64,
    pub m3_version: u64,
    pub m3_hash: [u8; 32],
    pub m3_floor: u64,
    pub primary_policy_sha256: [u8; 32],
    pub export_secondary_policy_sha256: [u8; 32],
    pub import_secondary_policy_sha256: [u8; 32],
    pub export_acl_sha256: [u8; 32],
    pub import_acl_sha256: [u8; 32],
    pub blocked_next_seq: u64,
    pub intent_state: IntentState,
    pub attempt_id: u64,
    pub prefix: u64,
}

fn set(tlv: &mut Tlv, field: &str, digests: &[[u8; 32]]) -> Result<(), RestoreError> {
    use RestoreError::InvalidRevocationSet;

    let size = digests
        .len()
        .checked_mul(32)
        .and_then(|n| n.checked_add(8))
        .filter(|&n| u32::try_from(n).is_ok())
        .ok_or(InvalidRevocationSet)?;
    let mut ordered = digests.to_vec();
    ordered.sort_unstable();
    if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(InvalidRevocationSet);
    }
    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(
        &u64::try_from(digests.len())
            .map_err(|_| InvalidRevocationSet)?
            .to_be_bytes(),
    );
    for digest in ordered {
        bytes.extend_from_slice(&digest);
    }
    tlv.bytes(field, &bytes);
    Ok(())
}

impl ControlState {
    /// Compromete observações completas de autoridade, configuração e cópia.
    ///
    /// # Errors
    /// Recusa conjuntos inválidos ou campos fora da faixa ou do estado atestado.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreError> {
        let mut tlv = Tlv::new(b"godmesh.federation.restore.controls.v1");
        tlv.key(&self.key)?;
        positive(self.generation, "generation")?;
        tlv.u64("generation", self.generation);
        tlv.node("root_node_id", &self.root_node_id)?;
        tlv.byte("authority_kind", self.authority_kind as u8);
        tlv.bytes("authority_ref", &self.authority_ref);
        set(&mut tlv, "origin_revocations", &self.origin_revocations)?;
        set(
            &mut tlv,
            "destination_revocations",
            &self.destination_revocations,
        )?;
        range(self.m3_epoch, "m3_epoch")?;
        positive(self.m3_version, "m3_version")?;
        positive(self.m3_floor, "m3_floor")?;
        tlv.u64("m3_epoch", self.m3_epoch);
        tlv.u64("m3_version", self.m3_version);
        tlv.bytes("m3_hash", &self.m3_hash);
        tlv.u64("m3_floor", self.m3_floor);
        tlv.bytes("primary_policy_sha256", &self.primary_policy_sha256);
        tlv.bytes(
            "export_secondary_policy_sha256",
            &self.export_secondary_policy_sha256,
        );
        tlv.bytes(
            "import_secondary_policy_sha256",
            &self.import_secondary_policy_sha256,
        );
        tlv.bytes("export_acl_sha256", &self.export_acl_sha256);
        tlv.bytes("import_acl_sha256", &self.import_acl_sha256);
        range(self.prefix, "prefix")?;
        check_suspension(self.prefix, self.blocked_next_seq)?;
        tlv.u64("blocked_next_seq", self.blocked_next_seq);
        check_intent(self.intent_state, self.attempt_id)?;
        tlv.intent(self.intent_state, self.attempt_id);
        tlv.u64("prefix", self.prefix);
        Ok(tlv.0)
    }

    /// # Errors
    /// Propaga falhas da preimagem.
    pub fn digest(&self) -> Result<[u8; 32], RestoreError> {
        self.canonical_bytes().map(sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fence {
    pub key: RestoreKey,
    pub fenced_generation: u64,
    pub cut_db_id: String,
    pub target_db_id: String,
    pub old_login: String,
    pub admission_closed: bool,
    pub nologin_confirmed: bool,
    pub grants_removed: bool,
    pub remaining_sessions: u64,
    pub unknown_effects: u64,
    pub fenced_at: i64,
}

impl Fence {
    /// # Errors
    /// Recusa fence incompleto ou campos inválidos.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreError> {
        if !(self.admission_closed && self.nologin_confirmed && self.grants_removed)
            || self.remaining_sessions != 0
            || self.unknown_effects != 0
        {
            return Err(RestoreError::InvalidFence);
        }
        let mut tlv = Tlv::new(b"godmesh.federation.restore.fence.v1");
        tlv.key(&self.key)?;
        positive(self.fenced_generation, "fenced_generation")?;
        tlv.u64("fenced_generation", self.fenced_generation);
        tlv.text("cut_db_id", &self.cut_db_id)?;
        tlv.text("target_db_id", &self.target_db_id)?;
        tlv.text("old_login", &self.old_login)?;
        tlv.byte("admission_closed", u8::from(self.admission_closed));
        tlv.byte("nologin_confirmed", u8::from(self.nologin_confirmed));
        tlv.byte("grants_removed", u8::from(self.grants_removed));
        tlv.u64("remaining_sessions", self.remaining_sessions);
        tlv.u64("unknown_effects", self.unknown_effects);
        tlv.i64("fenced_at", self.fenced_at);
        Ok(tlv.0)
    }

    /// # Errors
    /// Propaga falhas da preimagem.
    pub fn digest(&self) -> Result<[u8; 32], RestoreError> {
        self.canonical_bytes().map(sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    pub key: RestoreKey,
    pub target_generation: u64,
    pub high_water_sha256: [u8; 32],
    pub final_prefix: u64,
    pub proofs_sha256: [u8; 32],
    pub current_control_sha256: [u8; 32],
    pub blocked_next_seq: u64,
    pub intent_state: IntentState,
    pub attempt_id: u64,
}

impl Reconciliation {
    /// # Errors
    /// Recusa campos, prova vazia, suspensão ou intenção incoerentes.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreError> {
        let mut tlv = Tlv::new(b"godmesh.federation.restore.reconciled.v1");
        tlv.key(&self.key)?;
        positive(self.target_generation, "target_generation")?;
        tlv.u64("target_generation", self.target_generation);
        tlv.bytes("high_water_sha256", &self.high_water_sha256);
        range(self.final_prefix, "final_prefix")?;
        if self.final_prefix == 0 && self.proofs_sha256 != proofs_digest(0, [])? {
            return Err(RestoreError::InvalidCut);
        }
        tlv.u64("final_prefix", self.final_prefix);
        tlv.bytes("proofs_sha256", &self.proofs_sha256);
        tlv.bytes("current_control_sha256", &self.current_control_sha256);
        check_suspension(self.final_prefix, self.blocked_next_seq)?;
        tlv.u64("blocked_next_seq", self.blocked_next_seq);
        check_intent(self.intent_state, self.attempt_id)?;
        tlv.intent(self.intent_state, self.attempt_id);
        Ok(tlv.0)
    }

    /// # Errors
    /// Propaga falhas da preimagem.
    pub fn digest(&self) -> Result<[u8; 32], RestoreError> {
        self.canonical_bytes().map(sha256)
    }
}
