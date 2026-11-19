use std::{
    collections::BTreeMap,
    fmt,
    net::{IpAddr, SocketAddr},
    num::NonZeroUsize,
    time::Duration,
};

use axum::http::uri::Authority;
use godmesh_federation::{
    Genesis,
    config_bundle::{BundlePolicy, PROFILE, PinnedVoter, SoftwareVersion},
};

use crate::{
    config_bundle::{Role, ServicePolicy},
    config_https::HttpsLimits,
};

const PREFIX: &str = "GODMESH_CONFIG_";
const KEYS: &[&str] = &[
    "CLUSTER",
    "ROLE",
    "EPOCH",
    "VOTERS",
    "SOFTWARE_VERSION",
    "MAX_FRAME_BYTES",
    "BIND",
    "READERS",
    "SUBMITTERS",
    "RESOLVERS",
    "HANDSHAKE_LIMIT",
    "CONNECTION_LIMIT",
    "HANDSHAKE_TIMEOUT_MS",
    "HEADER_TIMEOUT_MS",
    "MAX_BODY_BYTES",
    "BODY_TIMEOUT_MS",
    "PULL_TIMEOUT_MS",
    "POLL_INTERVAL_MS",
    "MAX_ATTEMPTS",
    "RETRY_INTERVAL_MS",
    "LINK_RECHECK_INTERVAL_MS",
    "PUBLISHER_URL",
    "PUBLISHER_PIN",
];

#[derive(Debug)]
pub struct ConfigError(pub(crate) &'static str);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuração do plano HTTPS inválida: {}", self.0)
    }
}
impl std::error::Error for ConfigError {}

type Pins = BTreeMap<String, [u8; 32]>;

#[derive(Clone, Debug)]
pub struct Publisher {
    pub authority: Authority,
    pub node_id: String,
    pub public_key: [u8; 32],
}

impl Publisher {
    pub(crate) async fn address(&self) -> std::io::Result<SocketAddr> {
        let host = self
            .authority
            .host()
            .trim_start_matches('[')
            .trim_end_matches(']');
        let port = self.authority.port_u16().unwrap_or(443);
        tokio::net::lookup_host((host, port))
            .await?
            .next()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::AddrNotAvailable,
                    "publicador sem endereço",
                )
            })
    }
}

#[derive(Clone, Debug)]
pub struct ConfigPlaneConfig {
    pub policy: ServicePolicy,
    pub bind: SocketAddr,
    pub max_handshakes: NonZeroUsize,
    pub max_connections: NonZeroUsize,
    pub handshake_timeout: Duration,
    pub header_timeout: Duration,
    pub max_body_bytes: NonZeroUsize,
    pub body_timeout: Duration,
    pub pull_timeout: Duration,
    pub poll_interval: Duration,
    pub max_attempts: NonZeroUsize,
    pub retry_interval: Duration,
    pub link_recheck_interval: Duration,
    pub(crate) readers: Pins,
    pub(crate) submitters: Pins,
    pub(crate) resolvers: Pins,
    publisher: Option<Publisher>,
}

impl ConfigPlaneConfig {
    pub fn from_env() -> Result<Option<Self>, ConfigError> {
        for &key in KEYS {
            if matches!(
                std::env::var(format!("{PREFIX}{key}")),
                Err(std::env::VarError::NotUnicode(_))
            ) {
                return Err(ConfigError(key));
            }
        }
        Self::from_env_with(|key| std::env::var(key).ok())
    }

    pub fn from_env_with(
        get: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Self>, ConfigError> {
        let fields: BTreeMap<&str, String> = KEYS
            .iter()
            .filter_map(|&key| get(&format!("{PREFIX}{key}")).map(|v| (key, v)))
            .collect();
        if fields.is_empty() {
            return Ok(None);
        }
        let req = |key| fields.get(key).map(String::as_str).ok_or(ConfigError(key));
        let count = |key| {
            decimal(req(key)?)
                .and_then(|n| usize::try_from(n).ok())
                .and_then(NonZeroUsize::new)
                .ok_or(ConfigError(key))
        };
        let duration = |key| {
            decimal(req(key)?)
                .filter(|n| *n > 0)
                .map(Duration::from_millis)
                .filter(|d| std::time::Instant::now().checked_add(*d).is_some())
                .ok_or(ConfigError(key))
        };
        let role = match req("ROLE")? {
            "publisher" => Role::Publisher,
            "receiver" => Role::Receiver,
            _ => return Err(ConfigError("ROLE")),
        };
        let cluster = req("CLUSTER")?.to_owned();
        let epoch = decimal(req("EPOCH")?).ok_or(ConfigError("EPOCH"))?;
        let voters = pins(req("VOTERS")?, "VOTERS")?
            .into_iter()
            .map(|(node_id, key)| PinnedVoter {
                node_id,
                public_key: hex::encode(key),
            })
            .collect();
        let bundle = BundlePolicy {
            cluster,
            epoch,
            voters,
            max_frame_bytes: count("MAX_FRAME_BYTES")?,
        };
        bundle
            .validate()
            .map_err(|_| ConfigError("política M3-A"))?;
        let mut version = req("SOFTWARE_VERSION")?.split('.');
        let component = |part: Option<&str>| {
            part.filter(|v| v.len() == 1 || !v.starts_with('0'))
                .and_then(decimal)
        };
        let software = match (
            component(version.next()),
            component(version.next()),
            component(version.next()),
            version.next(),
        ) {
            (Some(a), Some(b), Some(c), None) => SoftwareVersion::new(a, b, c),
            _ => None,
        }
        .ok_or(ConfigError("SOFTWARE_VERSION"))?;
        let max_body_bytes = count("MAX_BODY_BYTES")?;
        if max_body_bytes < bundle.max_frame_bytes {
            return Err(ConfigError("MAX_BODY_BYTES < MAX_FRAME_BYTES"));
        }
        let publisher = match role {
            Role::Publisher => {
                if fields.contains_key("PUBLISHER_URL") || fields.contains_key("PUBLISHER_PIN") {
                    return Err(ConfigError(
                        "PUBLISHER_URL/PUBLISHER_PIN exclusivos do receptor",
                    ));
                }
                None
            }
            Role::Receiver => Some(publisher(req("PUBLISHER_URL")?, req("PUBLISHER_PIN")?)?),
        };
        let max_handshakes = count("HANDSHAKE_LIMIT")?;
        let max_connections = count("CONNECTION_LIMIT")?;
        if max_connections.get() > tokio::sync::Semaphore::MAX_PERMITS
            || max_handshakes.get() > tokio::sync::Semaphore::MAX_PERMITS
        {
            return Err(ConfigError("limite de conexões"));
        }
        Ok(Some(Self {
            policy: ServicePolicy {
                profile: PROFILE,
                bundle,
                role,
                software,
            },
            bind: req("BIND")?.parse().map_err(|_| ConfigError("BIND"))?,
            max_handshakes,
            max_connections,
            handshake_timeout: duration("HANDSHAKE_TIMEOUT_MS")?,
            header_timeout: duration("HEADER_TIMEOUT_MS")?,
            max_body_bytes,
            body_timeout: duration("BODY_TIMEOUT_MS")?,
            pull_timeout: duration("PULL_TIMEOUT_MS")?,
            poll_interval: duration("POLL_INTERVAL_MS")?,
            max_attempts: count("MAX_ATTEMPTS")?,
            retry_interval: duration("RETRY_INTERVAL_MS")?,
            link_recheck_interval: duration("LINK_RECHECK_INTERVAL_MS")?,
            readers: pins(req("READERS")?, "READERS")?,
            submitters: pins(req("SUBMITTERS")?, "SUBMITTERS")?,
            resolvers: pins(req("RESOLVERS")?, "RESOLVERS")?,
            publisher,
        }))
    }

    pub fn publisher(&self) -> Option<&Publisher> {
        self.publisher.as_ref()
    }

    pub fn https_limits(&self) -> HttpsLimits {
        HttpsLimits {
            max_handshakes: self.max_handshakes.get(),
            max_connections: self.max_connections.get(),
            handshake_timeout: self.handshake_timeout,
            header_timeout: self.header_timeout,
            max_body_bytes: self.max_body_bytes.get(),
            body_timeout: self.body_timeout,
            pull_timeout: self.pull_timeout,
        }
    }

    pub fn tls_pins(&self) -> Vec<[u8; 32]> {
        let mut pins: Vec<_> = self
            .readers
            .values()
            .chain(self.submitters.values())
            .chain(self.resolvers.values())
            .copied()
            .collect();
        pins.sort_unstable();
        pins.dedup();
        pins
    }
}

fn decimal(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn pins(raw: &str, name: &'static str) -> Result<Pins, ConfigError> {
    let parsed = Genesis::parse(raw).map_err(|_| ConfigError(name))?;
    parsed
        .nodes()
        .iter()
        .map(|pin| {
            if pin.public_key.bytes().any(|b| b.is_ascii_uppercase()) {
                return Err(ConfigError(name));
            }
            let mut key = [0; 32];
            hex::decode_to_slice(&pin.public_key, &mut key).map_err(|_| ConfigError(name))?;
            Ok((pin.node_id.clone(), key))
        })
        .collect()
}

fn publisher(url: &str, pin: &str) -> Result<Publisher, ConfigError> {
    let bad_url = ConfigError("PUBLISHER_URL");
    let authority = url
        .strip_prefix("https://")
        .filter(|s| !s.contains(['/', '?', '#', '@']))
        .and_then(|s| s.parse::<Authority>().ok())
        .ok_or(bad_url)?;
    let host = authority.host();
    let literal = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    if (authority.as_str().len() != host.len()
        && !authority.port_u16().is_some_and(|port| port > 0))
        || (literal.parse::<IpAddr>().is_err()
            && (host.is_empty()
                || !host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))))
    {
        return Err(ConfigError("PUBLISHER_URL"));
    }
    let pins = pins(pin, "PUBLISHER_PIN")?;
    if pins.len() != 1 {
        return Err(ConfigError("PUBLISHER_PIN"));
    }
    let (node_id, public_key) = pins
        .into_iter()
        .next()
        .ok_or(ConfigError("PUBLISHER_PIN"))?;
    Ok(Publisher {
        authority,
        node_id,
        public_key,
    })
}
