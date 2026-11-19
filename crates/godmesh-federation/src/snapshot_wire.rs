//! Controles JSON puros do perfil M4-B2-B. Escape, prefixos e FIN ficam no transporte.

use std::{fmt, num::NonZeroU32};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

pub use crate::m4a_wire::AckClass;

use crate::{identity, manifest::MAX_FIELD_LEN, snapshot::VerifiedSnapshot};

const MAX_SEQ: u64 = i64::MAX as u64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Namespace {
    pub cluster: String,
    pub tenant_id: String,
    pub stream_id: String,
    pub partition: u32,
    pub authority_epoch: u64,
}

#[derive(PartialEq, Eq)]
struct Address<'a> {
    request_id: &'a str,
    origin_node: &'a str,
    destination_node: &'a str,
    namespace: &'a Namespace,
}

// Os campos de correlação são planos no JSON de todos os controles.
macro_rules! control_fields {
    ($name:ident { $($field:ident: $ty:ty),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub request_id: String,
            pub origin_node: String,
            pub destination_node: String,
            pub namespace: Namespace,
            $(pub $field: $ty),+
        }

        impl $name {
            fn address(&self) -> Address<'_> {
                Address {
                    request_id: &self.request_id,
                    origin_node: &self.origin_node,
                    destination_node: &self.destination_node,
                    namespace: &self.namespace,
                }
            }
        }
    };
}

control_fields!(Request {
    base_seq: u64,
    receiver_max_snapshot_bytes: NonZeroU32,
});

control_fields!(Response {
    base_seq: u64,
    cut_seq: u64,
    package_bytes: NonZeroU32,
    source_max_snapshot_bytes: NonZeroU32,
    package_sha256: String,
});

control_fields!(Ack {
    cut_seq: u64,
    package_sha256: String,
    ack_class: AckClass,
});

control_fields!(ErrorMessage { error: ErrorCode });

/// Códigos remotos estáveis do perfil B2-B.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ScopeDenied,
    AuthorityBlocked,
    ResidenceDenied,
    HistoryUnavailable,
    SequenceGap,
    ProofMissing,
    ProofConflict,
    LimitExceeded,
    SnapshotConflict,
    StorageUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Control {
    #[serde(rename = "m4b.snapshot.request.v1")]
    Request(Request),
    #[serde(rename = "m4b.snapshot.response.v1")]
    Response(Response),
    #[serde(rename = "m4b.snapshot.error.v1")]
    Error(ErrorMessage),
    #[serde(rename = "m4b.snapshot.ack.v1")]
    Ack(Ack),
}

/// Falhas locais de decodificação ou validação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    MalformedControl,
    InvalidField(&'static str),
    LimitExceeded,
    CorrelationMismatch,
    LengthMismatch,
    HashMismatch,
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WireError {}

fn lower_hex(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_FIELD_LEN
}

impl Address<'_> {
    fn validate(&self) -> Result<(), WireError> {
        let ns = self.namespace;
        for (field, valid) in [
            ("request_id", lower_hex(self.request_id, 32)),
            ("origin_node", identity::is_node_id(self.origin_node)),
            (
                "destination_node",
                identity::is_node_id(self.destination_node)
                    && self.destination_node != self.origin_node,
            ),
            ("cluster", text(&ns.cluster)),
            ("tenant_id", text(&ns.tenant_id)),
            (
                "stream_id",
                text(&ns.stream_id) && ns.stream_id.strip_prefix("m4a.metadata.").is_some_and(text),
            ),
            ("partition", ns.partition == 0),
            (
                "authority_epoch",
                (1..=MAX_SEQ).contains(&ns.authority_epoch),
            ),
        ] {
            if !valid {
                return Err(WireError::InvalidField(field));
            }
        }
        Ok(())
    }
}

impl Request {
    fn validate(&self) -> Result<(), WireError> {
        self.address().validate()?;
        if self.base_seq > MAX_SEQ {
            return Err(WireError::InvalidField("base_seq"));
        }
        Ok(())
    }
}

impl Response {
    fn validate(&self) -> Result<(), WireError> {
        self.address().validate()?;
        if self.base_seq > MAX_SEQ {
            return Err(WireError::InvalidField("base_seq"));
        }
        if self.cut_seq <= self.base_seq || self.cut_seq > MAX_SEQ {
            return Err(WireError::InvalidField("cut_seq"));
        }
        if !lower_hex(&self.package_sha256, 64) {
            return Err(WireError::InvalidField("package_sha256"));
        }
        if self.package_bytes > self.source_max_snapshot_bytes {
            return Err(WireError::LimitExceeded);
        }
        Ok(())
    }
}

impl Ack {
    fn validate(&self) -> Result<(), WireError> {
        self.address().validate()?;
        if !(1..=MAX_SEQ).contains(&self.cut_seq) {
            return Err(WireError::InvalidField("cut_seq"));
        }
        if !lower_hex(&self.package_sha256, 64) {
            return Err(WireError::InvalidField("package_sha256"));
        }
        Ok(())
    }
}

impl Control {
    fn validate(&self) -> Result<(), WireError> {
        match self {
            Self::Request(request) => request.validate(),
            Self::Response(response) => response.validate(),
            Self::Ack(ack) => ack.validate(),
            Self::Error(error) => error.address().validate(),
        }
    }
}

fn check_size(bytes: &[u8], limit: NonZeroU32) -> Result<(), WireError> {
    if !u32::try_from(bytes.len()).is_ok_and(|size| size <= limit.get()) {
        return Err(WireError::LimitExceeded);
    }
    Ok(())
}

/// Lê um único controle JSON estrito, sem prefixo de comprimento.
///
/// # Errors
/// Recusa excesso do teto informado, estrutura inválida ou campo incompatível.
pub fn decode_control(bytes: &[u8], max_control_bytes: NonZeroU32) -> Result<Control, WireError> {
    #[derive(Deserialize)]
    struct Shape<'a> {
        #[serde(borrow)]
        namespace: &'a RawValue,
    }

    check_size(bytes, max_control_bytes)?;
    let json = std::str::from_utf8(bytes).map_err(|_| WireError::MalformedControl)?;
    let json = json.trim_start_matches([' ', '\r', '\n', '\t']);
    if !json.starts_with('{') {
        return Err(WireError::MalformedControl);
    }
    // O Serde pode aceitar arrays posicionais para structs derivadas.
    let shape: Shape<'_> = serde_json::from_str(json).map_err(|_| WireError::MalformedControl)?;
    if !shape.namespace.get().starts_with('{') {
        return Err(WireError::MalformedControl);
    }
    let control: Control = serde_json::from_str(json).map_err(|_| WireError::MalformedControl)?;
    control.validate()?;
    Ok(control)
}

/// Emite um controle JSON compacto, sem prefixo de comprimento.
///
/// # Errors
/// Recusa campos inválidos ou excesso do teto informado.
pub fn encode_control(
    control: &Control,
    max_control_bytes: NonZeroU32,
) -> Result<Vec<u8>, WireError> {
    control.validate()?;
    let bytes = serde_json::to_vec(control).map_err(|_| WireError::MalformedControl)?;
    check_size(&bytes, max_control_bytes)?;
    Ok(bytes)
}

/// Confere a resposta com o pedido original antes de ler ou alocar o pacote.
///
/// # Errors
/// Recusa campos inválidos, divergência de correlação ou pacote acima do teto.
/// Em sucesso, devolve o menor entre oferta original, teto declarado e teto local.
pub fn validate_response(
    request: &Request,
    response: &Response,
    local_max: NonZeroU32,
) -> Result<NonZeroU32, WireError> {
    request.validate()?;
    response.validate()?;
    if request.address() != response.address() || request.base_seq != response.base_seq {
        return Err(WireError::CorrelationMismatch);
    }
    let max = request
        .receiver_max_snapshot_bytes
        .min(response.source_max_snapshot_bytes)
        .min(local_max);
    if response.package_bytes > max {
        return Err(WireError::LimitExceeded);
    }
    Ok(max)
}

/// Verifica comprimento e SHA-256 do pacote binário exato para a troca original.
///
/// # Errors
/// Retorna falhas de resposta ou de divergência de comprimento ou hash.
pub fn validate_package(
    request: &Request,
    response: &Response,
    package: &[u8],
    local_max: NonZeroU32,
) -> Result<(), WireError> {
    validate_response(request, response, local_max)?;
    if u32::try_from(package.len()).ok() != Some(response.package_bytes.get()) {
        return Err(WireError::LengthMismatch);
    }
    if hex::encode(Sha256::digest(package)) != response.package_sha256 {
        return Err(WireError::HashMismatch);
    }
    Ok(())
}

/// Confere a correlação com o manifesto B1 autenticado do mesmo pacote.
///
/// # Errors
/// Recusa resposta inválida ou divergência de escopo, base ou corte.
pub fn validate_manifest(
    response: &Response,
    verified: &VerifiedSnapshot<'_>,
) -> Result<(), WireError> {
    response.validate()?;
    let manifest = verified.manifest();
    let ns = &response.namespace;
    if manifest.origin_node != response.origin_node
        || manifest.destination_node != response.destination_node
        || manifest.cluster != ns.cluster
        || manifest.tenant_id != ns.tenant_id
        || manifest.stream_id != ns.stream_id
        || manifest.partition != ns.partition
        || manifest.authority_epoch != ns.authority_epoch
        || manifest.base_seq != response.base_seq
        || manifest.cut_seq != response.cut_seq
    {
        return Err(WireError::CorrelationMismatch);
    }
    Ok(())
}

/// Confere tupla, corte e hash do ACK contra a resposta original.
///
/// # Errors
/// Recusa campos inválidos ou correlação divergente.
pub fn validate_ack(response: &Response, ack: &Ack) -> Result<(), WireError> {
    response.validate()?;
    ack.validate()?;
    if response.address() != ack.address()
        || response.cut_seq != ack.cut_seq
        || response.package_sha256 != ack.package_sha256
    {
        return Err(WireError::CorrelationMismatch);
    }
    Ok(())
}

/// Confere a tupla completa do erro contra o pedido original.
///
/// # Errors
/// Recusa campos inválidos ou correlação divergente.
pub fn validate_error(request: &Request, error: &ErrorMessage) -> Result<(), WireError> {
    request.validate()?;
    error.address().validate()?;
    if request.address() != error.address() {
        return Err(WireError::CorrelationMismatch);
    }
    Ok(())
}
