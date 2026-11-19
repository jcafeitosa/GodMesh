//! Controles da abertura M5. O transporte administra a stream e a barreira do token.

use std::{fmt, num::NonZeroU32};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Open {
    pub request_id: String,
    pub route_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accept {
    pub request_id: String,
    pub route_id: String,
    pub max_datagram_bytes: NonZeroU32,
    pub ready_token: String,
}

impl Accept {
    /// Decodifica os 16 bytes enviados na mesma stream antes dos datagramas.
    ///
    /// # Errors
    /// Recusa token cujo comprimento ou alfabeto diverge do wire.
    pub fn token_bytes(&self) -> Result<[u8; 16], WireError> {
        if !lower_hex_16(&self.ready_token) {
            return Err(WireError::InvalidField("ready_token"));
        }
        let mut bytes = [0; 16];
        hex::decode_to_slice(&self.ready_token, &mut bytes)
            .map_err(|_| WireError::InvalidField("ready_token"))?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refuse {
    pub request_id: String,
    pub route_id: String,
    pub error: ErrorCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    RouteDenied,
    AuthorityBlocked,
    QuotaExceeded,
    RouteUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Control {
    #[serde(rename = "m5.relay.open.v1")]
    Open(Open),
    #[serde(rename = "m5.relay.accept.v1")]
    Accept(Accept),
    #[serde(rename = "m5.relay.refuse.v1")]
    Refuse(Refuse),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    MalformedControl,
    InvalidField(&'static str),
    InvalidLength,
    LimitExceeded,
    UnexpectedControl,
    CorrelationMismatch,
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WireError {}

fn lower_hex_16(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_ids(request_id: &str, route_id: &str) -> Result<(), WireError> {
    if !lower_hex_16(request_id) {
        return Err(WireError::InvalidField("request_id"));
    }
    if !(1..=64).contains(&route_id.len())
        || !route_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(WireError::InvalidField("route_id"));
    }
    Ok(())
}

impl Control {
    fn ids(&self) -> (&str, &str) {
        match self {
            Self::Open(value) => (&value.request_id, &value.route_id),
            Self::Accept(value) => (&value.request_id, &value.route_id),
            Self::Refuse(value) => (&value.request_id, &value.route_id),
        }
    }

    fn validate(&self) -> Result<(), WireError> {
        let (request_id, route_id) = self.ids();
        validate_ids(request_id, route_id)?;
        if let Self::Accept(accept) = self {
            accept.token_bytes()?;
        }
        Ok(())
    }
}

/// Confere o comprimento anunciado antes da alocação ou leitura do corpo.
///
/// # Errors
/// Recusa zero e comprimentos acima do teto aplicável à mensagem.
pub fn frame_length(prefix: [u8; 4], limit: NonZeroU32) -> Result<NonZeroU32, WireError> {
    let length = NonZeroU32::new(u32::from_be_bytes(prefix)).ok_or(WireError::InvalidLength)?;
    if length > limit {
        return Err(WireError::LimitExceeded);
    }
    Ok(length)
}

fn check_control_size(bytes: &[u8], limit: NonZeroU32) -> Result<(), WireError> {
    if !u32::try_from(bytes.len()).is_ok_and(|length| length <= limit.get()) {
        return Err(WireError::LimitExceeded);
    }
    Ok(())
}

/// Decodifica um único objeto JSON, sem o prefixo de comprimento.
///
/// # Errors
/// Recusa JSON inválido, estrutura fora do schema, campos inválidos e excesso de tamanho.
pub fn decode_control(bytes: &[u8], limit: NonZeroU32) -> Result<Control, WireError> {
    check_control_size(bytes, limit)?;
    let json = std::str::from_utf8(bytes).map_err(|_| WireError::MalformedControl)?;
    if !json
        .trim_start_matches([' ', '\r', '\n', '\t'])
        .starts_with('{')
    {
        return Err(WireError::MalformedControl);
    }
    let control: Control = serde_json::from_str(json).map_err(|_| WireError::MalformedControl)?;
    control.validate()?;
    Ok(control)
}

/// Emite um controle validado em JSON compacto, sem o prefixo de comprimento.
///
/// # Errors
/// Recusa campos inválidos e excesso de tamanho.
pub fn encode_control(control: &Control, limit: NonZeroU32) -> Result<Vec<u8>, WireError> {
    control.validate()?;
    let bytes = serde_json::to_vec(control).map_err(|_| WireError::MalformedControl)?;
    check_control_size(&bytes, limit)?;
    Ok(bytes)
}

/// Confere a resposta na mesma stream contra o pedido e o teto fixado para a rota.
///
/// # Errors
/// Recusa pedido como resposta, divergências de correlação e aceitações com teto incompatível.
pub fn validate_reply(
    request: &Open,
    reply: &Control,
    expected_datagram_bytes: NonZeroU32,
) -> Result<(), WireError> {
    validate_ids(&request.request_id, &request.route_id)?;
    reply.validate()?;
    if matches!(reply, Control::Open(_)) {
        return Err(WireError::UnexpectedControl);
    }
    if reply.ids() != (request.request_id.as_str(), request.route_id.as_str()) {
        return Err(WireError::CorrelationMismatch);
    }
    if let Control::Accept(accept) = reply
        && accept.max_datagram_bytes != expected_datagram_bytes
    {
        return Err(WireError::InvalidField("max_datagram_bytes"));
    }
    Ok(())
}
