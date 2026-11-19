use std::{
    collections::BTreeMap,
    fmt,
    num::{NonZeroU32, NonZeroUsize},
    time::{Duration, Instant},
};

use godmesh_federation::identity::{is_node_id, parse_public_key, verify_node_id};

use crate::replication::Namespace;

const PREFIX: &str = "GODMESH_FEDERATION_M4_";
const FIELDS: &[&str] = &[
    "ROLE",
    "CLUSTER",
    "TENANT_ID",
    "STREAM_ID",
    "PARTITION",
    "AUTHORITY_EPOCH",
    "ORIGIN_PIN",
    "DESTINATION_PINS",
    "MAX_FRAME_BYTES",
    "MAX_ENVELOPE_BYTES",
    "MAX_EVENTS",
    "MAX_STREAMS_PER_CONNECTION",
    "MAX_ATTEMPTS",
    "ATTEMPT_TIMEOUT_MS",
    "RETRY_INTERVAL_MS",
    "ACL_RECHECK_INTERVAL_MS",
    "PULL_INTERVAL_MS",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Origin,
    Receiver,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    pub node_id: String,
    pub public_key: String,
}

impl Pin {
    fn parse(value: &str) -> Option<Self> {
        let (id, key) = value.split_once(':')?;
        if !is_node_id(id)
            || key.len() != 64
            || !key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || parse_public_key(key).is_err()
            || !verify_node_id(id, key)
        {
            return None;
        }
        Some(Self {
            node_id: id.to_owned(),
            public_key: key.to_owned(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct ReplicaConfig {
    pub role: Role,
    pub cluster: String,
    pub namespace: Namespace,
    pub authority_epoch: i64,
    pub origin_pin: Pin,
    pub destination_pins: Vec<Pin>,
    pub max_frame_bytes: NonZeroU32,
    pub max_envelope_bytes: NonZeroUsize,
    pub max_events: NonZeroUsize,
    pub max_streams_per_connection: NonZeroUsize,
    pub max_attempts: NonZeroUsize,
    pub attempt_timeout: Duration,
    pub retry_interval: Duration,
    pub acl_recheck_interval: Duration,
    pub pull_interval: Option<Duration>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub field: &'static str,
    pub reason: &'static str,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "m4_config_invalid {PREFIX}{} {}",
            self.field, self.reason
        )
    }
}
impl std::error::Error for ConfigError {}

fn invalid(field: &'static str) -> ConfigError {
    ConfigError {
        field,
        reason: "campo ausente ou inválido para o perfil",
    }
}

impl ReplicaConfig {
    pub fn from_env() -> Result<Option<Self>, ConfigError> {
        let vars = std::env::vars_os()
            .filter(|(key, _)| key.as_encoded_bytes().starts_with(PREFIX.as_bytes()))
            .map(|(key, value)| {
                let key = key.into_string().map_err(|_| invalid("*"))?;
                let field = FIELDS
                    .iter()
                    .copied()
                    .find(|field| key.strip_prefix(PREFIX) == Some(*field))
                    .ok_or_else(|| invalid("*"))?;
                Ok((key, value.into_string().map_err(|_| invalid(field))?))
            })
            .collect::<Result<Vec<_>, ConfigError>>()?;
        Self::from_env_with(vars)
    }

    pub fn from_env_with(
        vars: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Option<Self>, ConfigError> {
        let mut fields = BTreeMap::new();
        for (name, value) in vars {
            let Some(field) = name.strip_prefix(PREFIX) else {
                continue;
            };
            let field =
                FIELDS
                    .iter()
                    .copied()
                    .find(|known| *known == field)
                    .ok_or(ConfigError {
                        field: "*",
                        reason: "sufixo desconhecido",
                    })?;
            if value.trim().is_empty() || fields.insert(field, value).is_some() {
                return Err(invalid(field));
            }
        }
        if fields.is_empty() {
            return Ok(None);
        }
        let text = |field| {
            fields
                .get(field)
                .map(String::as_str)
                .ok_or_else(|| invalid(field))
        };
        let role = match text("ROLE")? {
            "origin" => Role::Origin,
            "receiver" => Role::Receiver,
            _ => return Err(invalid("ROLE")),
        };
        let partition: i32 = decimal(&fields, "PARTITION")?;
        let authority_epoch: i64 = decimal(&fields, "AUTHORITY_EPOCH")?;
        if partition != 0 {
            return Err(invalid("PARTITION"));
        }
        if authority_epoch <= 0 {
            return Err(invalid("AUTHORITY_EPOCH"));
        }
        let stream = text("STREAM_ID")?;
        if !stream
            .strip_prefix("m4a.metadata.")
            .is_some_and(|suffix| !suffix.trim().is_empty())
        {
            return Err(invalid("STREAM_ID"));
        }
        let origin_pin = Pin::parse(text("ORIGIN_PIN")?).ok_or_else(|| invalid("ORIGIN_PIN"))?;
        let (destination_pins, pull_interval) = match role {
            Role::Origin => {
                if fields.contains_key("PULL_INTERVAL_MS") {
                    return Err(invalid("PULL_INTERVAL_MS"));
                }
                let pins = text("DESTINATION_PINS")?
                    .split(',')
                    .map(Pin::parse)
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| invalid("DESTINATION_PINS"))?;
                if pins.len() != 2
                    || pins[0].node_id == pins[1].node_id
                    || pins[0].public_key == pins[1].public_key
                    || pins.iter().any(|pin| {
                        pin.node_id == origin_pin.node_id || pin.public_key == origin_pin.public_key
                    })
                {
                    return Err(invalid("DESTINATION_PINS"));
                }
                (pins, None)
            }
            Role::Receiver => {
                if fields.contains_key("DESTINATION_PINS") {
                    return Err(invalid("DESTINATION_PINS"));
                }
                (Vec::new(), Some(duration(&fields, "PULL_INTERVAL_MS")?))
            }
        };
        let config = Self {
            role,
            cluster: text("CLUSTER")?.to_owned(),
            namespace: Namespace {
                origin_node: origin_pin.node_id.clone(),
                tenant_id: text("TENANT_ID")?.to_owned(),
                stream_id: stream.to_owned(),
                partition,
            },
            authority_epoch,
            origin_pin,
            destination_pins,
            max_frame_bytes: decimal(&fields, "MAX_FRAME_BYTES")?,
            max_envelope_bytes: decimal(&fields, "MAX_ENVELOPE_BYTES")?,
            max_events: decimal(&fields, "MAX_EVENTS")?,
            max_streams_per_connection: decimal(&fields, "MAX_STREAMS_PER_CONNECTION")?,
            max_attempts: decimal(&fields, "MAX_ATTEMPTS")?,
            attempt_timeout: duration(&fields, "ATTEMPT_TIMEOUT_MS")?,
            retry_interval: duration(&fields, "RETRY_INTERVAL_MS")?,
            acl_recheck_interval: duration(&fields, "ACL_RECHECK_INTERVAL_MS")?,
            pull_interval,
        };
        let frame = usize::try_from(config.max_frame_bytes.get())
            .map_err(|_| invalid("MAX_FRAME_BYTES"))?;
        if config.max_envelope_bytes.get() > frame {
            return Err(invalid("MAX_ENVELOPE_BYTES"));
        }
        if config.control_bytes() > frame {
            return Err(invalid("MAX_FRAME_BYTES"));
        }
        Ok(Some(config))
    }

    fn control_bytes(&self) -> usize {
        use serde_json::json;
        let request_id = "0".repeat(32);
        let namespace = json!({
            "origin_node": self.namespace.origin_node, "tenant_id": self.namespace.tenant_id,
            "stream_id": self.namespace.stream_id, "partition": self.namespace.partition,
            "authority_epoch": self.authority_epoch,
        });
        let destination = format!("gmn_{}", "0".repeat(64));
        let own = json!({
            "frame_bytes": self.max_frame_bytes, "envelope_bytes": self.max_envelope_bytes,
            "event_count": self.max_events,
        });
        let peer_max = json!({
            "frame_bytes": u32::MAX, "envelope_bytes": usize::MAX, "event_count": usize::MAX,
        });
        let (source, offer) = match self.role {
            Role::Origin => (&own, &peer_max),
            Role::Receiver => (&peer_max, &own),
        };
        [
            json!({"type":"m4a.pull.v1", "request_id":request_id, "namespace":namespace,
                "destination":destination, "after_seq":i64::MAX, "receiver_limits":offer}),
            json!({"type":"m4a.batch.v1", "request_id":request_id, "namespace":namespace,
                "destination":destination, "after_seq":i64::MAX,
                "source_limits":source, "effective_limits":own, "events":[], "has_more":false}),
            json!({"type":"m4a.ack.v1", "request_id":request_id, "namespace":namespace,
                "destination":destination, "through_seq":i64::MAX, "ack_class":"persisted_local"}),
            json!({"type":"m4a.error.v1", "request_id":request_id, "error":"storage_unavailable"}),
        ]
        .iter()
        .map(|frame| frame.to_string().len())
        .max()
        .unwrap_or(0)
    }
}

fn decimal<T: std::str::FromStr>(
    fields: &BTreeMap<&str, String>,
    field: &'static str,
) -> Result<T, ConfigError> {
    let value = fields.get(field).ok_or_else(|| invalid(field))?;
    if !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(invalid(field));
    }
    value.parse().map_err(|_| invalid(field))
}

fn duration(fields: &BTreeMap<&str, String>, field: &'static str) -> Result<Duration, ConfigError> {
    let ms: std::num::NonZeroU64 = decimal(fields, field)?;
    let duration = Duration::from_millis(ms.get());
    Instant::now()
        .checked_add(duration)
        .ok_or_else(|| invalid(field))?;
    Ok(duration)
}
