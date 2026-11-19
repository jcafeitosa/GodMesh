//! Codec JSON puro do perfil M4-A2. O transporte cuida do prefixo u32 e do FIN.

use std::{
    fmt,
    num::{NonZeroU32, NonZeroUsize},
};

use serde::{Deserialize, Serialize, de::IgnoredAny};
use serde_json::value::RawValue;

use crate::{event::SignedEvent, identity};

/// Chave completa da série no fio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Namespace {
    pub origin_node: String,
    pub tenant_id: String,
    pub stream_id: String,
    pub partition: i32,
    pub authority_epoch: i64,
}

/// Tetos positivos para o JSON completo, cada envelope original e a contagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub frame_bytes: NonZeroU32,
    pub envelope_bytes: NonZeroUsize,
    pub event_count: NonZeroUsize,
}

impl Limits {
    #[must_use]
    pub fn component_min(self, other: Self) -> Self {
        Self {
            frame_bytes: self.frame_bytes.min(other.frame_bytes),
            envelope_bytes: self.envelope_bytes.min(other.envelope_bytes),
            event_count: self.event_count.min(other.event_count),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pull {
    pub request_id: String,
    pub namespace: Namespace,
    pub destination: String,
    pub after_seq: i64,
    pub receiver_limits: Limits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch<'a> {
    pub request_id: String,
    pub namespace: Namespace,
    pub destination: String,
    pub after_seq: i64,
    pub source_limits: Limits,
    pub effective_limits: Limits,
    /// Fatias JSON brutas, incluindo whitespace antes e depois de cada objeto.
    pub events: Vec<&'a [u8]>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AckClass {
    PersistedLocal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ack {
    pub request_id: String,
    pub namespace: Namespace,
    pub destination: String,
    pub through_seq: i64,
    pub ack_class: AckClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ScopeDenied,
    ResidenceDenied,
    AuthorityBlocked,
    ProofMissing,
    ProofConflict,
    SequenceGap,
    HistoryUnavailable,
    LimitExceeded,
    StorageUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrorMessage {
    pub request_id: String,
    pub error: ErrorCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message<'a> {
    Pull(Pull),
    Batch(Batch<'a>),
    Ack(Ack),
    Error(ErrorMessage),
}

/// Falha local do codec ou de correlação. `ErrorCode` é o código remoto no fio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    MalformedBody,
    InvalidField(&'static str),
    LimitExceeded,
    LimitsMismatch,
    CorrelationMismatch,
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for WireError {}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum WireMessage {
    #[serde(rename = "m4a.pull.v1")]
    Pull(Pull),
    #[serde(rename = "m4a.batch.v1")]
    Batch(BatchHeader),
    #[serde(rename = "m4a.ack.v1")]
    Ack(Ack),
    #[serde(rename = "m4a.error.v1")]
    Error(ErrorMessage),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchHeader {
    request_id: String,
    namespace: Namespace,
    destination: String,
    after_seq: i64,
    source_limits: Limits,
    effective_limits: Limits,
    #[serde(rename = "events", skip_serializing)]
    _events: IgnoredAny,
    has_more: bool,
}

impl BatchHeader {
    fn from_batch(batch: &Batch<'_>) -> Self {
        Self {
            request_id: batch.request_id.clone(),
            namespace: batch.namespace.clone(),
            destination: batch.destination.clone(),
            after_seq: batch.after_seq,
            source_limits: batch.source_limits,
            effective_limits: batch.effective_limits,
            _events: IgnoredAny,
            has_more: batch.has_more,
        }
    }

    fn into_batch(self, events: Vec<&[u8]>) -> Batch<'_> {
        Batch {
            request_id: self.request_id,
            namespace: self.namespace,
            destination: self.destination,
            after_seq: self.after_seq,
            source_limits: self.source_limits,
            effective_limits: self.effective_limits,
            events,
            has_more: self.has_more,
        }
    }
}

// Serde aceita arrays posicionais para structs derivadas. Conferimos as
// formas originais antes de decodificar campos, mantendo o array de eventos bruto.
#[derive(Deserialize)]
struct BodyShape<'a> {
    #[serde(borrow)]
    namespace: Option<&'a RawValue>,
    #[serde(borrow)]
    receiver_limits: Option<&'a RawValue>,
    #[serde(borrow)]
    source_limits: Option<&'a RawValue>,
    #[serde(borrow)]
    effective_limits: Option<&'a RawValue>,
    #[serde(borrow)]
    events: Option<&'a RawValue>,
}

fn trim_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'));
    start.map_or(&[], |start| &bytes[start..])
}

fn require_object(bytes: &[u8]) -> Result<(), WireError> {
    if trim_start(bytes).first() != Some(&b'{') {
        return Err(WireError::MalformedBody);
    }
    Ok(())
}

fn check_len(len: usize, limit: NonZeroU32) -> Result<(), WireError> {
    if u32::try_from(len).map_or(true, |len| len > limit.get()) {
        return Err(WireError::LimitExceeded);
    }
    Ok(())
}

fn validate_request_id(id: &str) -> Result<(), WireError> {
    if id.len() != 32
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(WireError::InvalidField("request_id"));
    }
    Ok(())
}

fn validate_address(namespace: &Namespace, destination: &str) -> Result<(), WireError> {
    if !identity::is_node_id(&namespace.origin_node) {
        return Err(WireError::InvalidField("origin_node"));
    }
    if namespace.tenant_id.trim().is_empty() {
        return Err(WireError::InvalidField("tenant_id"));
    }
    if namespace.stream_id.trim().is_empty() {
        return Err(WireError::InvalidField("stream_id"));
    }
    if namespace.partition < 0 {
        return Err(WireError::InvalidField("partition"));
    }
    if namespace.authority_epoch <= 0 {
        return Err(WireError::InvalidField("authority_epoch"));
    }
    if !identity::is_node_id(destination) {
        return Err(WireError::InvalidField("destination"));
    }
    Ok(())
}

fn validate_message(message: &Message<'_>) -> Result<(), WireError> {
    match message {
        Message::Pull(pull) => {
            validate_request_id(&pull.request_id)?;
            validate_address(&pull.namespace, &pull.destination)?;
            if pull.after_seq < 0 {
                return Err(WireError::InvalidField("after_seq"));
            }
        }
        Message::Batch(batch) => {
            validate_request_id(&batch.request_id)?;
            validate_address(&batch.namespace, &batch.destination)?;
            if batch.after_seq < 0 {
                return Err(WireError::InvalidField("after_seq"));
            }
            if batch.events.is_empty() && batch.has_more {
                return Err(WireError::InvalidField("has_more"));
            }
        }
        Message::Ack(ack) => {
            validate_request_id(&ack.request_id)?;
            validate_address(&ack.namespace, &ack.destination)?;
            if ack.through_seq <= 0 {
                return Err(WireError::InvalidField("through_seq"));
            }
        }
        Message::Error(error) => validate_request_id(&error.request_id)?,
    }
    Ok(())
}

fn decode_event(frame: &[u8]) -> Result<SignedEvent, WireError> {
    #[derive(Deserialize)]
    struct EventShape<'a> {
        #[serde(borrow)]
        position: &'a RawValue,
    }

    require_object(frame)?;
    let shape: EventShape<'_> =
        serde_json::from_slice(frame).map_err(|_| WireError::MalformedBody)?;
    require_object(shape.position.get().as_bytes())?;
    let event: SignedEvent = serde_json::from_slice(frame).map_err(|_| WireError::MalformedBody)?;
    if i32::try_from(event.position.partition).is_err() {
        return Err(WireError::InvalidField("partition"));
    }
    if !(1..=i64::MAX as u64).contains(&event.position.authority_epoch) {
        return Err(WireError::InvalidField("authority_epoch"));
    }
    if !(1..=i64::MAX as u64).contains(&event.position.seq) {
        return Err(WireError::InvalidField("seq"));
    }
    Ok(event)
}

fn event_slices(array: &RawValue) -> Result<Vec<&[u8]>, WireError> {
    let mut rest = array
        .get()
        .as_bytes()
        .strip_prefix(b"[")
        .ok_or(WireError::MalformedBody)?;
    let mut events = Vec::new();
    if trim_start(rest) == b"]" {
        return Ok(events);
    }
    loop {
        // O parser JSON localiza o fim do valor. A fatia vai até o próximo
        // delimitador e inclui o whitespace que RawValue sozinho descartaria.
        let mut parser = serde_json::Deserializer::from_slice(rest).into_iter::<&RawValue>();
        parser
            .next()
            .ok_or(WireError::MalformedBody)?
            .map_err(|_| WireError::MalformedBody)?;
        let end = parser.byte_offset();
        let delimiter = trim_start(&rest[end..]);
        let item = &rest[..rest.len() - delimiter.len()];
        decode_event(item)?;
        events.push(item);
        if delimiter == b"]" {
            return Ok(events);
        }
        rest = delimiter
            .strip_prefix(b",")
            .ok_or(WireError::MalformedBody)?;
    }
}

/// Decodifica exatamente um corpo JSON. O transporte já deve ter conferido o prefixo.
///
/// # Errors
/// Recusa corpos fora do teto, do formato ou das faixas do perfil.
pub fn decode_body(body: &[u8], local_frame_bytes: NonZeroU32) -> Result<Message<'_>, WireError> {
    check_len(body.len(), local_frame_bytes)?;
    let json = std::str::from_utf8(body).map_err(|_| WireError::MalformedBody)?;
    require_object(body)?;
    let shape: BodyShape<'_> = serde_json::from_str(json).map_err(|_| WireError::MalformedBody)?;
    for object in [
        shape.namespace,
        shape.receiver_limits,
        shape.source_limits,
        shape.effective_limits,
    ]
    .into_iter()
    .flatten()
    {
        require_object(object.get().as_bytes())?;
    }
    let wire = serde_json::from_str(json).map_err(|_| WireError::MalformedBody)?;
    let message = match wire {
        WireMessage::Pull(pull) => Message::Pull(pull),
        WireMessage::Batch(header) => {
            let events = event_slices(shape.events.ok_or(WireError::MalformedBody)?)?;
            Message::Batch(header.into_batch(events))
        }
        WireMessage::Ack(ack) => Message::Ack(ack),
        WireMessage::Error(error) => Message::Error(error),
    };
    validate_message(&message)?;
    Ok(message)
}

fn check_events(events: &[&[u8]], limits: Limits) -> Result<(), WireError> {
    if events.len() > limits.event_count.get()
        || events
            .iter()
            .any(|event| event.len() > limits.envelope_bytes.get())
    {
        return Err(WireError::LimitExceeded);
    }
    Ok(())
}

fn validate_positions(batch: &Batch<'_>) -> Result<(), WireError> {
    let namespace = &batch.namespace;
    let mut seq = batch.after_seq;
    for frame in &batch.events {
        let event = decode_event(frame)?;
        let position = event.position;
        if position.origin_node != namespace.origin_node
            || event.tenant_id != namespace.tenant_id
            || position.stream_id != namespace.stream_id
            || i32::try_from(position.partition).ok() != Some(namespace.partition)
            || i64::try_from(position.authority_epoch).ok() != Some(namespace.authority_epoch)
        {
            return Err(WireError::CorrelationMismatch);
        }
        seq = seq.checked_add(1).ok_or(WireError::InvalidField("seq"))?;
        if i64::try_from(position.seq).ok() != Some(seq) {
            return Err(WireError::InvalidField("seq"));
        }
    }
    Ok(())
}

fn encode_batch(batch: &Batch<'_>, local_frame_bytes: NonZeroU32) -> Result<Vec<u8>, WireError> {
    check_events(&batch.events, batch.effective_limits)?;
    let mut body = serde_json::to_vec(&WireMessage::Batch(BatchHeader::from_batch(batch)))
        .map_err(|_| WireError::MalformedBody)?;
    let _ = body.pop(); // A serialização do cabeçalho termina com o fechamento do objeto.
    body.extend_from_slice(b",\"events\":[");
    let limit = local_frame_bytes.min(batch.effective_limits.frame_bytes);
    let overhead = body
        .len()
        .checked_add(2)
        .and_then(|len| len.checked_add(batch.events.len().saturating_sub(1)))
        .ok_or(WireError::LimitExceeded)?;
    let full_len = batch
        .events
        .iter()
        .try_fold(overhead, |len, event| len.checked_add(event.len()))
        .ok_or(WireError::LimitExceeded)?;
    check_len(full_len, limit)?;
    validate_positions(batch)?;
    for (index, event) in batch.events.iter().enumerate() {
        if index != 0 {
            body.push(b',');
        }
        body.extend_from_slice(event);
    }
    body.extend_from_slice(b"]}");
    Ok(body)
}

/// Codifica o corpo JSON sem o prefixo, preservando os envelopes originais do Batch.
///
/// # Errors
/// Recusa mensagens fora do perfil ou além do teto aplicável.
pub fn encode_body(
    message: &Message<'_>,
    local_frame_bytes: NonZeroU32,
) -> Result<Vec<u8>, WireError> {
    validate_message(message)?;
    let wire = match message {
        Message::Pull(pull) => WireMessage::Pull(pull.clone()),
        Message::Batch(batch) => return encode_batch(batch, local_frame_bytes),
        Message::Ack(ack) => WireMessage::Ack(ack.clone()),
        Message::Error(error) => WireMessage::Error(error.clone()),
    };
    let body = serde_json::to_vec(&wire).map_err(|_| WireError::MalformedBody)?;
    check_len(body.len(), local_frame_bytes)?;
    Ok(body)
}

/// Confere a resposta contra o Pull da mesma stream e o tamanho do JSON recebido.
///
/// # Errors
/// Recusa correlação divergente, mínimos incorretos, limites excedidos ou posições
/// fora do namespace e da sequência contígua esperada.
pub fn validate_batch(pull: &Pull, batch: &Batch<'_>, body_len: usize) -> Result<(), WireError> {
    if batch.request_id != pull.request_id
        || batch.namespace != pull.namespace
        || batch.destination != pull.destination
        || batch.after_seq != pull.after_seq
    {
        return Err(WireError::CorrelationMismatch);
    }
    let limits = pull.receiver_limits.component_min(batch.source_limits);
    if limits != batch.effective_limits {
        return Err(WireError::LimitsMismatch);
    }
    if batch.events.is_empty() && batch.has_more {
        return Err(WireError::InvalidField("has_more"));
    }
    check_len(body_len, limits.frame_bytes)?;
    check_events(&batch.events, limits)?;
    validate_positions(batch)
}

/// Confere o ACK contra a requisição e a última sequência enviada na stream.
///
/// # Errors
/// Retorna `CorrelationMismatch` se a identidade da requisição ou a faixa divergir.
pub fn validate_ack(pull: &Pull, ack: &Ack, through_seq: i64) -> Result<(), WireError> {
    if ack.request_id != pull.request_id
        || ack.namespace != pull.namespace
        || ack.destination != pull.destination
        || ack.through_seq != through_seq
        || through_seq <= pull.after_seq
    {
        return Err(WireError::CorrelationMismatch);
    }
    Ok(())
}

/// Confere o ID da resposta de erro contra a requisição da mesma stream.
///
/// # Errors
/// Retorna `CorrelationMismatch` se os IDs divergirem.
pub fn validate_error(pull: &Pull, error: &ErrorMessage) -> Result<(), WireError> {
    if error.request_id != pull.request_id {
        return Err(WireError::CorrelationMismatch);
    }
    Ok(())
}
