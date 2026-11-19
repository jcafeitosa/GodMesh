//! Framing e verificação pura do snapshot de metadados M4-B1.

use std::{
    fmt,
    num::{NonZeroU32, NonZeroUsize},
    str::FromStr,
};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

use crate::{
    NodeIdentity,
    event::{EventPositionClaim, SignedEvent},
    identity,
    manifest::{MAX_FIELD_LEN, push_field},
};

const PROFILE: u64 = 1;
const MAX_POSITION: u64 = i64::MAX as u64;
const MANIFEST_DOMAIN: &[u8] = b"godmesh.federation.snapshot.v1";
const EVENTS_DOMAIN: &[u8] = b"godmesh.federation.snapshot.events.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotIdentity {
    pub node_id: String,
    pub public_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotLimits {
    pub max_package_bytes: NonZeroUsize,
    pub max_manifest_bytes: NonZeroU32,
    pub max_event_bytes: NonZeroU32,
    pub max_events: NonZeroU32,
}

/// Expectativas locais, obtidas independentemente do pacote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPolicy {
    pub cluster: String,
    pub origin: SnapshotIdentity,
    pub destination: SnapshotIdentity,
    pub tenant_id: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    pub retention_class: String,
    pub limits: SnapshotLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotManifest {
    pub profile: u64,
    pub cluster: String,
    pub origin_node: String,
    pub destination_node: String,
    pub tenant_id: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
    pub base_seq: u64,
    pub cut_seq: u64,
    pub event_count: u32,
    pub not_before: i64,
    pub expires_at: i64,
    pub events_digest: String,
    pub signature: String,
}

impl SnapshotManifest {
    /// Codifica a preimagem assinada. A assinatura fica fora do corpo.
    ///
    /// # Errors
    /// Retorna `InvalidField` para texto acima do teto ou digest malformado.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SnapshotError> {
        let mut bytes = MANIFEST_DOMAIN.to_vec();
        push_field(&mut bytes, "profile", &self.profile.to_be_bytes());
        for (field, value) in [
            ("cluster", &self.cluster),
            ("origin_node", &self.origin_node),
            ("destination_node", &self.destination_node),
            ("tenant_id", &self.tenant_id),
            ("stream_id", &self.stream_id),
        ] {
            if value.len() > MAX_FIELD_LEN {
                return Err(SnapshotError::InvalidField(field));
            }
            push_field(&mut bytes, field, value.as_bytes());
        }
        push_field(&mut bytes, "partition", &self.partition.to_be_bytes());
        push_field(
            &mut bytes,
            "authority_epoch",
            &self.authority_epoch.to_be_bytes(),
        );
        push_field(&mut bytes, "base_seq", &self.base_seq.to_be_bytes());
        push_field(&mut bytes, "cut_seq", &self.cut_seq.to_be_bytes());
        push_field(&mut bytes, "event_count", &self.event_count.to_be_bytes());
        push_field(&mut bytes, "not_before", &self.not_before.to_be_bytes());
        push_field(&mut bytes, "expires_at", &self.expires_at.to_be_bytes());
        push_field(
            &mut bytes,
            "events_digest",
            &decode_digest(&self.events_digest)?,
        );
        Ok(bytes)
    }

    /// Assina o manifesto com a identidade explícita da origem.
    ///
    /// # Errors
    /// Retorna `ScopeMismatch` para outro signatário ou erro da preimagem.
    pub fn sign(&mut self, origin: &NodeIdentity) -> Result<(), SnapshotError> {
        if origin.node_id() != self.origin_node {
            return Err(SnapshotError::ScopeMismatch);
        }
        self.signature = origin.sign_hex(&self.canonical_bytes()?);
        Ok(())
    }
}

/// Prova autenticada para a política informada, com a janela assinada intacta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedSnapshot<'a> {
    manifest: SnapshotManifest,
    events: Vec<&'a [u8]>,
}

impl<'a> VerifiedSnapshot<'a> {
    #[must_use]
    pub fn manifest(&self) -> &SnapshotManifest {
        &self.manifest
    }

    /// Base exclusiva e corte inclusivo.
    #[must_use]
    pub fn cut(&self) -> (u64, u64) {
        (self.manifest.base_seq, self.manifest.cut_seq)
    }

    /// Início inclusivo e fim exclusivo, em segundos Unix.
    #[must_use]
    pub fn window(&self) -> (i64, i64) {
        (self.manifest.not_before, self.manifest.expires_at)
    }

    #[must_use]
    pub fn events(&self) -> &[&'a [u8]] {
        &self.events
    }
}

/// Erros de prova individual incluem o índice a partir de zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    InvalidPolicy,
    PackageTooLarge,
    ManifestTooLarge,
    MalformedManifest,
    UnsupportedProfile,
    InvalidField(&'static str),
    InvalidCut,
    InvalidWindow,
    TooManyEvents,
    ScopeMismatch,
    EventTooLarge,
    MalformedPackage,
    ManifestSignatureInvalid,
    EventsDigestMismatch,
    InvalidEvent(usize),
    EventScopeMismatch(usize),
    SequenceMismatch(usize),
    EventSignatureInvalid(usize),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SnapshotError {}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_FIELD_LEN
}

fn stream(value: &str) -> bool {
    text(value) && value.strip_prefix("m4a.metadata.").is_some_and(text)
}

fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn decode_digest(value: &str) -> Result<[u8; 32], SnapshotError> {
    let mut bytes = [0; 32];
    if !lower_hex(value, 64) {
        return Err(SnapshotError::InvalidField("events_digest"));
    }
    hex::decode_to_slice(value, &mut bytes)
        .map_err(|_| SnapshotError::InvalidField("events_digest"))?;
    Ok(bytes)
}

fn valid_policy(policy: &SnapshotPolicy) -> bool {
    text(&policy.cluster)
        && text(&policy.tenant_id)
        && stream(&policy.stream_id)
        && policy.partition == 0
        && (1..=MAX_POSITION).contains(&policy.authority_epoch)
        && text(&policy.retention_class)
        && policy.origin.node_id != policy.destination.node_id
        && identity::verify_node_id(&policy.origin.node_id, &policy.origin.public_key)
        && identity::verify_node_id(&policy.destination.node_id, &policy.destination.public_key)
}

fn matches_policy(manifest: &SnapshotManifest, policy: &SnapshotPolicy) -> bool {
    manifest.cluster == policy.cluster
        && manifest.origin_node == policy.origin.node_id
        && manifest.destination_node == policy.destination.node_id
        && manifest.tenant_id == policy.tenant_id
        && manifest.stream_id == policy.stream_id
        && manifest.partition == policy.partition
        && manifest.authority_epoch == policy.authority_epoch
}

// A desserialização verifica o tipo inteiro antes de qualquer teste de faixa.
// RawValue preserva -0 e números além de u64 sem arredondar nem coerir.
struct Integer<'a>(&'a str);

impl<'de: 'a, 'a> Deserialize<'de> for Integer<'a> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <&RawValue>::deserialize(deserializer)?.get();
        let digits = raw.strip_prefix('-').unwrap_or(raw);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(D::Error::custom("esperado inteiro JSON"));
        }
        Ok(Self(raw))
    }
}

impl Integer<'_> {
    fn parse<T: FromStr>(&self) -> Option<T> {
        self.0.parse().ok()
    }
}

fn object<'a, T: Deserialize<'a>>(json: &'a str) -> Option<T> {
    let json = json.trim_start_matches([' ', '\t', '\r', '\n']);
    if !json.starts_with('{') {
        return None;
    }
    serde_json::from_str(json).ok()
}

#[derive(Deserialize)]
struct ProfileProbe<'a> {
    #[serde(borrow)]
    profile: Integer<'a>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireManifest<'a> {
    #[serde(rename = "profile", borrow)]
    _profile: Integer<'a>,
    cluster: String,
    origin_node: String,
    destination_node: String,
    tenant_id: String,
    stream_id: String,
    #[serde(borrow)]
    partition: Integer<'a>,
    #[serde(borrow)]
    authority_epoch: Integer<'a>,
    #[serde(borrow)]
    base_seq: Integer<'a>,
    #[serde(borrow)]
    cut_seq: Integer<'a>,
    #[serde(borrow)]
    event_count: Integer<'a>,
    #[serde(borrow)]
    not_before: Integer<'a>,
    #[serde(borrow)]
    expires_at: Integer<'a>,
    events_digest: String,
    signature: String,
}

fn parse_manifest(frame: &[u8], max_events: NonZeroU32) -> Result<SnapshotManifest, SnapshotError> {
    use SnapshotError::*;

    let json = std::str::from_utf8(frame).map_err(|_| MalformedManifest)?;
    let probe: ProfileProbe<'_> = object(json).ok_or(MalformedManifest)?;
    if probe.profile.parse::<u64>().ok_or(MalformedManifest)? != PROFILE {
        return Err(UnsupportedProfile);
    }
    let wire: WireManifest<'_> = object(json).ok_or(MalformedManifest)?;
    for (field, valid) in [
        ("cluster", text(&wire.cluster)),
        ("origin_node", identity::is_node_id(&wire.origin_node)),
        (
            "destination_node",
            identity::is_node_id(&wire.destination_node)
                && wire.destination_node != wire.origin_node,
        ),
        ("tenant_id", text(&wire.tenant_id)),
        ("stream_id", stream(&wire.stream_id)),
    ] {
        if !valid {
            return Err(InvalidField(field));
        }
    }
    let partition = wire
        .partition
        .parse::<u32>()
        .filter(|&p| p == 0)
        .ok_or(InvalidField("partition"))?;
    let positive = |number: &Integer<'_>, field| {
        number
            .parse::<u64>()
            .filter(|value| (1..=MAX_POSITION).contains(value))
            .ok_or(InvalidField(field))
    };
    let authority_epoch = positive(&wire.authority_epoch, "authority_epoch")?;
    let base_seq = wire
        .base_seq
        .parse::<u64>()
        .filter(|&value| value <= MAX_POSITION)
        .ok_or(InvalidField("base_seq"))?;
    let cut_seq = positive(&wire.cut_seq, "cut_seq")?;
    let event_count = wire
        .event_count
        .parse::<u32>()
        .filter(|&value| value > 0)
        .ok_or(InvalidField("event_count"))?;
    let not_before = wire
        .not_before
        .parse::<i64>()
        .ok_or(InvalidField("not_before"))?;
    let expires_at = wire
        .expires_at
        .parse::<i64>()
        .ok_or(InvalidField("expires_at"))?;
    if !lower_hex(&wire.events_digest, 64) {
        return Err(InvalidField("events_digest"));
    }
    if !lower_hex(&wire.signature, 128) {
        return Err(InvalidField("signature"));
    }
    if cut_seq.checked_sub(base_seq) != Some(u64::from(event_count)) {
        return Err(InvalidCut);
    }
    if not_before >= expires_at {
        return Err(InvalidWindow);
    }
    if event_count > max_events.get() {
        return Err(TooManyEvents);
    }
    Ok(SnapshotManifest {
        profile: PROFILE,
        cluster: wire.cluster,
        origin_node: wire.origin_node,
        destination_node: wire.destination_node,
        tenant_id: wire.tenant_id,
        stream_id: wire.stream_id,
        partition,
        authority_epoch,
        base_seq,
        cut_seq,
        event_count,
        not_before,
        expires_at,
        events_digest: wire.events_digest,
        signature: wire.signature,
    })
}

fn read_frame<'a>(
    remaining: &mut &'a [u8],
    limit: NonZeroU32,
    malformed: SnapshotError,
    too_large: SnapshotError,
) -> Result<&'a [u8], SnapshotError> {
    let (prefix, content) = remaining.split_first_chunk::<4>().ok_or(malformed)?;
    let size = u32::from_be_bytes(*prefix);
    if size > limit.get() {
        return Err(too_large);
    }
    if size == 0 {
        return Err(malformed);
    }
    let (frame, rest) = content
        .split_at_checked(usize::try_from(size).map_err(|_| malformed)?)
        .ok_or(malformed)?;
    *remaining = rest;
    Ok(frame)
}

fn event_size(event: &[u8]) -> Result<u32, SnapshotError> {
    if event.is_empty() {
        return Err(SnapshotError::MalformedPackage);
    }
    u32::try_from(event.len()).map_err(|_| SnapshotError::EventTooLarge)
}

/// Calcula o digest da sequência de comprimentos e bytes originais.
///
/// # Errors
/// Recusa quantidades além de u32, frames vazios ou comprimentos além de u32.
pub fn events_digest(events: &[&[u8]]) -> Result<[u8; 32], SnapshotError> {
    let count = u32::try_from(events.len()).map_err(|_| SnapshotError::TooManyEvents)?;
    let mut digest = Sha256::new();
    digest.update(EVENTS_DOMAIN);
    digest.update(count.to_be_bytes());
    for event in events {
        digest.update(event_size(event)?.to_be_bytes());
        digest.update(event);
    }
    Ok(digest.finalize().into())
}

/// Emite o manifesto e as provas originais com os respectivos prefixos u32 BE.
///
/// # Errors
/// Recusa política, manifesto ou provas inválidos e excesso dos limites.
/// Confere as assinaturas e o escopo antes de alocar o pacote de saída.
pub fn encode_package(
    manifest: &SnapshotManifest,
    events: &[&[u8]],
    policy: &SnapshotPolicy,
) -> Result<Vec<u8>, SnapshotError> {
    use SnapshotError::*;

    if !valid_policy(policy) {
        return Err(InvalidPolicy);
    }
    let limits = &policy.limits;
    let json = serde_json::to_vec(manifest).map_err(|_| MalformedManifest)?;
    let manifest_size = u32::try_from(json.len()).map_err(|_| ManifestTooLarge)?;
    if manifest_size > limits.max_manifest_bytes.get() {
        return Err(ManifestTooLarge);
    }
    let manifest = parse_manifest(&json, limits.max_events)?;
    if !matches_policy(&manifest, policy) {
        return Err(ScopeMismatch);
    }
    if u32::try_from(events.len()).map_err(|_| TooManyEvents)? != manifest.event_count {
        return Err(InvalidCut);
    }
    let mut total = json.len().checked_add(4).ok_or(PackageTooLarge)?;
    for event in events {
        if event_size(event)? > limits.max_event_bytes.get() {
            return Err(EventTooLarge);
        }
        total = total
            .checked_add(4)
            .and_then(|n| n.checked_add(event.len()))
            .ok_or(PackageTooLarge)?;
    }
    if total > limits.max_package_bytes.get() {
        return Err(PackageTooLarge);
    }
    authenticate_snapshot(&manifest, events, policy)?;
    let mut package = Vec::with_capacity(total);
    package.extend_from_slice(&manifest_size.to_be_bytes());
    package.extend_from_slice(&json);
    for event in events {
        package.extend_from_slice(&event_size(event)?.to_be_bytes());
        package.extend_from_slice(event);
    }
    Ok(package)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireEvent<'a> {
    #[serde(borrow)]
    position: &'a RawValue,
    tenant_id: String,
    retention_class: String,
    #[serde(borrow)]
    issued_at: Integer<'a>,
    node_id: String,
    signature: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePosition<'a> {
    origin_node: String,
    stream_id: String,
    #[serde(borrow)]
    partition: Integer<'a>,
    #[serde(borrow)]
    authority_epoch: Integer<'a>,
    #[serde(borrow)]
    seq: Integer<'a>,
}

fn parse_event(frame: &[u8]) -> Option<SignedEvent> {
    let wire: WireEvent<'_> = object(std::str::from_utf8(frame).ok()?)?;
    let position: WirePosition<'_> = object(wire.position.get())?;
    let event = SignedEvent {
        position: EventPositionClaim {
            origin_node: position.origin_node,
            stream_id: position.stream_id,
            partition: position.partition.parse()?,
            authority_epoch: position.authority_epoch.parse()?,
            seq: position.seq.parse()?,
        },
        tenant_id: wire.tenant_id,
        retention_class: wire.retention_class,
        issued_at: wire.issued_at.parse()?,
        node_id: wire.node_id,
        signature: wire.signature,
    };
    event.validate_fields().ok()?;
    if !identity::is_node_id(&event.position.origin_node)
        || !(1..=MAX_POSITION).contains(&event.position.authority_epoch)
        || !(1..=MAX_POSITION).contains(&event.position.seq)
        || !lower_hex(&event.signature, 128)
    {
        return None;
    }
    Some(event)
}

fn verify_events(
    manifest: &SnapshotManifest,
    frames: &[&[u8]],
    policy: &SnapshotPolicy,
) -> Result<(), SnapshotError> {
    use SnapshotError::*;

    let mut expected = manifest.base_seq;
    for (index, frame) in frames.iter().enumerate() {
        let event = parse_event(frame).ok_or(InvalidEvent(index))?;
        if event.position.origin_node != manifest.origin_node
            || event.node_id != manifest.origin_node
            || event.tenant_id != manifest.tenant_id
            || event.position.stream_id != manifest.stream_id
            || event.position.partition != manifest.partition
            || event.position.authority_epoch != manifest.authority_epoch
            || event.retention_class != policy.retention_class
        {
            return Err(EventScopeMismatch(index));
        }
        expected = expected.checked_add(1).ok_or(SequenceMismatch(index))?;
        if event.position.seq != expected {
            return Err(SequenceMismatch(index));
        }
        event
            .verify_signature_against(&policy.origin.public_key)
            .map_err(|_| EventSignatureInvalid(index))?;
    }
    Ok(())
}

fn authenticate_snapshot(
    manifest: &SnapshotManifest,
    events: &[&[u8]],
    policy: &SnapshotPolicy,
) -> Result<(), SnapshotError> {
    use SnapshotError::*;

    identity::verify_signature(
        &policy.origin.public_key,
        &manifest.canonical_bytes()?,
        &manifest.signature,
    )
    .map_err(|_| ManifestSignatureInvalid)?;
    if events_digest(events)? != decode_digest(&manifest.events_digest)? {
        return Err(EventsDigestMismatch);
    }
    verify_events(manifest, events, policy)
}

/// Autentica o pacote e devolve fatias dos envelopes originais, na ordem do corte.
///
/// # Errors
/// Retorna a primeira falha segundo as etapas da SPEC M4-B1.
pub fn verify_snapshot<'a>(
    package: &'a [u8],
    policy: &SnapshotPolicy,
) -> Result<VerifiedSnapshot<'a>, SnapshotError> {
    use SnapshotError::*;

    if !valid_policy(policy) {
        return Err(InvalidPolicy);
    }
    if package.len() > policy.limits.max_package_bytes.get() {
        return Err(PackageTooLarge);
    }
    let mut remaining = package;
    let manifest_frame = read_frame(
        &mut remaining,
        policy.limits.max_manifest_bytes,
        MalformedManifest,
        ManifestTooLarge,
    )?;
    let manifest = parse_manifest(manifest_frame, policy.limits.max_events)?;
    if !matches_policy(&manifest, policy) {
        return Err(ScopeMismatch);
    }
    // Crescer apenas após extrair um frame completo de bytes já disponíveis.
    let mut events = Vec::new();
    for _ in 0..manifest.event_count {
        events.push(read_frame(
            &mut remaining,
            policy.limits.max_event_bytes,
            MalformedPackage,
            EventTooLarge,
        )?);
    }
    if !remaining.is_empty() {
        return Err(MalformedPackage);
    }

    authenticate_snapshot(&manifest, &events, policy)?;
    Ok(VerifiedSnapshot { manifest, events })
}
