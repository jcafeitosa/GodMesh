//! Prova portátil de ingresso M1C-A, verificada para a política e o instante do chamador.

use std::{collections::HashSet, fmt, marker::PhantomData, num::NonZeroUsize};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};
use sha2::{Digest, Sha256};

use crate::{
    GenesisNode, NodeIdentity, SignedManifest,
    approval::is_join_request_id,
    identity::{self, SIGNATURE_LEN},
    manifest::{MAX_FIELD_LEN, push_field},
};

const VERSION: u64 = 2;
const APPROVAL_DOMAIN: &[u8] = b"godmesh.federation.join.approval.v2";
const MANIFEST_DIGEST_DOMAIN: &[u8] = b"godmesh.federation.join.manifest-digest.v2";
const LINK_DIGEST_DOMAIN: &[u8] = b"godmesh.federation.join.link-digest.v2";

/// Emissor declarado pelo certificado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum JoinIssuerV2 {
    Genesis { node_id: String },
    Parent { digest: String },
}

/// Certificado portátil v2 ainda sujeito à verificação da cadeia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinCertificateV2 {
    pub join_request_id: String,
    pub cluster: String,
    pub candidate_node_id: String,
    pub candidate_public_key: String,
    pub manifest_digest: String,
    #[serde(deserialize_with = "object")]
    pub issuer: JoinIssuerV2,
    pub can_approve: bool,
    pub not_before: i64,
    pub expires_at: i64,
    pub epoch: u64,
    pub signature: String,
}

impl JoinCertificateV2 {
    /// Codifica os campos cobertos pela assinatura na ordem definida pela SPEC.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = APPROVAL_DOMAIN.to_vec();
        let (issuer_kind, issuer_ref) = match &self.issuer {
            JoinIssuerV2::Genesis { node_id } => ("genesis", node_id),
            JoinIssuerV2::Parent { digest } => ("parent", digest),
        };
        for (tag, value) in [
            ("join_request_id", self.join_request_id.as_str()),
            ("cluster", &self.cluster),
            ("candidate_node_id", &self.candidate_node_id),
            ("candidate_public_key", &self.candidate_public_key),
            ("manifest_digest", &self.manifest_digest),
            ("issuer_kind", issuer_kind),
            ("issuer_ref", issuer_ref),
        ] {
            push_field(&mut out, tag, value.as_bytes());
        }
        push_field(&mut out, "can_approve", &[u8::from(self.can_approve)]);
        push_field(&mut out, "not_before", &self.not_before.to_be_bytes());
        push_field(&mut out, "expires_at", &self.expires_at.to_be_bytes());
        push_field(&mut out, "epoch", &self.epoch.to_be_bytes());
        out
    }

    /// Assina com a identidade explícita. A cadeia estabelece a autoridade do emissor.
    pub fn sign(&mut self, identity: &NodeIdentity) {
        self.signature = identity.sign_hex(&self.canonical_bytes());
    }

    /// Identifica o certificado exato, incluindo sua assinatura.
    ///
    /// # Errors
    /// Retorna erro de hex se a assinatura não puder ser decodificada em 64 bytes.
    pub fn digest(&self) -> Result<String, hex::FromHexError> {
        signed_digest(LINK_DIGEST_DOMAIN, &self.canonical_bytes(), &self.signature)
    }
}

/// Identifica o manifesto assinado, independentemente da codificação JSON.
///
/// # Errors
/// Retorna erro de hex se a assinatura não puder ser decodificada em 64 bytes.
pub fn manifest_digest(manifest: &SignedManifest) -> Result<String, hex::FromHexError> {
    signed_digest(
        MANIFEST_DIGEST_DOMAIN,
        &manifest.canonical_bytes(),
        &manifest.signature,
    )
}

fn signed_digest(domain: &[u8], body: &[u8], signature: &str) -> Result<String, hex::FromHexError> {
    let mut signature_bytes = [0; SIGNATURE_LEN];
    hex::decode_to_slice(signature, &mut signature_bytes)?;
    let mut input = domain.to_vec();
    push_field(&mut input, "body", body);
    push_field(&mut input, "signature", &signature_bytes);
    Ok(hex::encode(Sha256::digest(input)))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinLinkV2 {
    #[serde(deserialize_with = "object")]
    pub certificate: JoinCertificateV2,
    pub manifest: SignedManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JoinChainV2 {
    pub version: u64,
    #[serde(deserialize_with = "link_objects")]
    pub links: Vec<JoinLinkV2>,
}

#[derive(Deserialize)]
struct VersionProbe {
    version: u64,
}

// Structs derivadas pelo Serde também aceitam sequências posicionais.
// Restringimos a entrada a mapas sem perder a detecção de campos duplicados.
struct StrictObject<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for StrictObject<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = StrictObject<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("um objeto")
            }

            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(StrictObject)
            }
        }

        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

fn object<'de, T: Deserialize<'de>, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
    StrictObject::deserialize(deserializer).map(|object| object.0)
}

fn link_objects<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<JoinLinkV2>, D::Error> {
    let links = Vec::<StrictObject<JoinLinkV2>>::deserialize(deserializer)?;
    Ok(links.into_iter().map(|link| link.0).collect())
}

/// Identidade esperada da folha, fornecida por uma fonte independente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedJoinIdentity {
    pub node_id: String,
    pub public_key: String,
}

/// Política explícita para um cluster. Os limites são inclusivos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinChainPolicy {
    pub cluster: String,
    pub roots: Vec<GenesisNode>,
    pub max_depth: NonZeroUsize,
    pub max_frame_bytes: NonZeroUsize,
}

/// Cadeia verificada para o instante e a política informados ao verificador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedJoinChain {
    cluster: String,
    root: GenesisNode,
    leaf: ExpectedJoinIdentity,
    leaf_digest: String,
    depth: usize,
    window: (i64, i64),
    can_approve: bool,
}

impl VerifiedJoinChain {
    #[must_use]
    pub fn cluster(&self) -> &str {
        &self.cluster
    }

    #[must_use]
    pub fn root(&self) -> &GenesisNode {
        &self.root
    }

    #[must_use]
    pub fn leaf(&self) -> &ExpectedJoinIdentity {
        &self.leaf
    }

    #[must_use]
    pub fn leaf_digest(&self) -> &str {
        &self.leaf_digest
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Início inclusivo e fim exclusivo, em segundos Unix.
    #[must_use]
    pub fn window(&self) -> (i64, i64) {
        self.window
    }

    #[must_use]
    pub fn can_approve(&self) -> bool {
        self.can_approve
    }
}

/// Falha determinística. As variantes relativas a elos incluem o índice a partir de zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinChainError {
    InvalidPolicy,
    InvalidExpectedIdentity,
    FrameTooLarge,
    MalformedFrame,
    UnsupportedVersion,
    EmptyChain,
    DepthExceeded,
    InvalidField(usize),
    ClusterMismatch(usize),
    InvalidWindow(usize),
    UnknownRoot(usize),
    InvalidIssuerPosition(usize),
    ParentDigestMismatch(usize),
    DelegationDenied(usize),
    RepeatedIdentity(usize),
    InvalidManifest(usize),
    ManifestBindingMismatch(usize),
    ManifestDigestMismatch(usize),
    InvalidSignature(usize),
    NotYetValid(usize),
    Expired(usize),
    ChildWindowOutsideParent(usize),
    CandidateMismatch,
}

impl fmt::Display for JoinChainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for JoinChainError {}

fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn valid_identity(node_id: &str, public_key: &str) -> bool {
    identity::is_node_id(node_id)
        && lower_hex(public_key, 64)
        && identity::verify_node_id(node_id, public_key)
}

fn valid_cluster(cluster: &str) -> bool {
    cluster.len() <= MAX_FIELD_LEN && !cluster.trim().is_empty()
}

fn valid_policy(policy: &JoinChainPolicy) -> bool {
    let mut ids = HashSet::new();
    valid_cluster(&policy.cluster)
        && !policy.roots.is_empty()
        && policy.roots.iter().all(|root| {
            valid_identity(&root.node_id, &root.public_key) && ids.insert(root.node_id.as_str())
        })
}

fn validate_certificate(
    cert: &JoinCertificateV2,
    cluster: &str,
    index: usize,
) -> Result<(), JoinChainError> {
    let valid_issuer = match &cert.issuer {
        JoinIssuerV2::Genesis { node_id } => identity::is_node_id(node_id),
        JoinIssuerV2::Parent { digest } => lower_hex(digest, 64),
    };
    if !is_join_request_id(&cert.join_request_id)
        || !valid_cluster(&cert.cluster)
        || !valid_identity(&cert.candidate_node_id, &cert.candidate_public_key)
        || !lower_hex(&cert.manifest_digest, 64)
        || !valid_issuer
        || cert.epoch > i64::MAX as u64
        || !lower_hex(&cert.signature, 128)
    {
        return Err(JoinChainError::InvalidField(index));
    }
    if cert.cluster != cluster {
        return Err(JoinChainError::ClusterMismatch(index));
    }
    if cert.not_before >= cert.expires_at {
        return Err(JoinChainError::InvalidWindow(index));
    }
    Ok(())
}

fn verify_manifest(link: &JoinLinkV2, index: usize) -> Result<(), JoinChainError> {
    let manifest = &link.manifest;
    let cert = &link.certificate;
    let invalid = |_| JoinChainError::InvalidManifest(index);
    manifest.to_frame().map_err(invalid)?;
    manifest.verify().map_err(invalid)?;
    if manifest.node_id != cert.candidate_node_id
        || manifest.public_key != cert.candidate_public_key
        || manifest.cluster != cert.cluster
    {
        return Err(JoinChainError::ManifestBindingMismatch(index));
    }
    if manifest_digest(manifest).map_err(|_| JoinChainError::InvalidManifest(index))?
        != cert.manifest_digest
    {
        return Err(JoinChainError::ManifestDigestMismatch(index));
    }
    Ok(())
}

fn decode_chain(frame: &[u8], policy: &JoinChainPolicy) -> Result<JoinChainV2, JoinChainError> {
    if frame.len() > policy.max_frame_bytes.get() {
        return Err(JoinChainError::FrameTooLarge);
    }
    let json = std::str::from_utf8(frame).map_err(|_| JoinChainError::MalformedFrame)?;
    let StrictObject(probe) = serde_json::from_str::<StrictObject<VersionProbe>>(json)
        .map_err(|_| JoinChainError::MalformedFrame)?;
    if probe.version != VERSION {
        return Err(JoinChainError::UnsupportedVersion);
    }
    let StrictObject(chain) = serde_json::from_str::<StrictObject<JoinChainV2>>(json)
        .map_err(|_| JoinChainError::MalformedFrame)?;
    if chain.links.len() > policy.max_depth.get() {
        return Err(JoinChainError::DepthExceeded);
    }
    Ok(chain)
}

struct VerifiedPrefix<'a> {
    root: &'a GenesisNode,
    last: &'a JoinCertificateV2,
    digest: String,
    window: (i64, i64),
}

/// Verifica os vínculos, as assinaturas e a janela da cadeia a partir da raiz configurada.
///
/// # Errors
/// Retorna a primeira falha na ordem definida pela SPEC M1C-A.
pub fn verify_join_chain(
    frame: &[u8],
    expected: &ExpectedJoinIdentity,
    now: i64,
    policy: &JoinChainPolicy,
) -> Result<VerifiedJoinChain, JoinChainError> {
    if !valid_policy(policy) {
        return Err(JoinChainError::InvalidPolicy);
    }
    if !valid_identity(&expected.node_id, &expected.public_key) {
        return Err(JoinChainError::InvalidExpectedIdentity);
    }
    let chain = decode_chain(frame, policy)?;
    let mut prefix: Option<VerifiedPrefix<'_>> = None;
    let mut seen = HashSet::new();
    for (index, link) in chain.links.iter().enumerate() {
        let cert = &link.certificate;
        validate_certificate(cert, &policy.cluster, index)?;
        let (root, issuer_key) = match (&prefix, &cert.issuer) {
            (None, JoinIssuerV2::Genesis { node_id }) => {
                let root = policy
                    .roots
                    .iter()
                    .find(|root| root.node_id == *node_id)
                    .ok_or(JoinChainError::UnknownRoot(index))?;
                seen.insert(root.node_id.as_str());
                (root, root.public_key.as_str())
            }
            (Some(parent), JoinIssuerV2::Parent { digest }) => {
                if digest != &parent.digest {
                    return Err(JoinChainError::ParentDigestMismatch(index));
                }
                if !parent.last.can_approve {
                    return Err(JoinChainError::DelegationDenied(index));
                }
                (parent.root, parent.last.candidate_public_key.as_str())
            }
            _ => return Err(JoinChainError::InvalidIssuerPosition(index)),
        };
        if !seen.insert(cert.candidate_node_id.as_str()) {
            return Err(JoinChainError::RepeatedIdentity(index));
        }
        verify_manifest(link, index)?;
        identity::verify_signature(issuer_key, &cert.canonical_bytes(), &cert.signature)
            .map_err(|_| JoinChainError::InvalidSignature(index))?;
        if now < cert.not_before {
            return Err(JoinChainError::NotYetValid(index));
        }
        if now >= cert.expires_at {
            return Err(JoinChainError::Expired(index));
        }
        let mut window = (cert.not_before, cert.expires_at);
        if let Some(parent) = &prefix {
            if window.0 < parent.last.not_before || window.1 > parent.last.expires_at {
                return Err(JoinChainError::ChildWindowOutsideParent(index));
            }
            window = (window.0.max(parent.window.0), window.1.min(parent.window.1));
        }
        let digest = cert
            .digest()
            .map_err(|_| JoinChainError::InvalidSignature(index))?;
        prefix = Some(VerifiedPrefix {
            root,
            last: cert,
            digest,
            window,
        });
    }

    let prefix = prefix.ok_or(JoinChainError::EmptyChain)?;
    if prefix.last.candidate_node_id != expected.node_id
        || prefix.last.candidate_public_key != expected.public_key
    {
        return Err(JoinChainError::CandidateMismatch);
    }
    Ok(VerifiedJoinChain {
        cluster: policy.cluster.clone(),
        root: prefix.root.clone(),
        leaf: expected.clone(),
        leaf_digest: prefix.digest,
        depth: chain.links.len(),
        window: prefix.window,
        can_approve: prefix.last.can_approve,
    })
}
