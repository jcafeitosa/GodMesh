use std::{fmt, future::Future, pin::Pin};

use axum::http::{HeaderMap, HeaderValue, StatusCode, header, uri::PathAndQuery};
use godmesh_federation::config_bundle::authenticate_bundle;
use sqlx::{Postgres, Transaction};
use tokio::{
    sync::watch,
    time::{Instant, timeout_at},
};

use super::{
    ConfigPlane, Peer, Publisher,
    api::{PATH, etag, json_media, parse_remote_error, read_etag},
    limit_statement_time,
};
use crate::{
    config_bundle::{Role, WriteError, WriteOutcome},
    portable_join::PortableJoinError,
};

pub struct PullResponse {
    pub peer: Peer,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    Network,
    Timeout,
    Pin,
    Protocol,
    InvalidBody,
}

pub type PullFuture<'a> =
    Pin<Box<dyn Future<Output = Result<PullResponse, TransportError>> + Send + 'a>>;

/// O adaptador autentica a conexão e devolve sua identidade com a resposta.
pub trait PullClient: Send + Sync {
    fn get<'a>(
        &'a self,
        publisher: &'a Publisher,
        target: &'a PathAndQuery,
        conditional: Option<&'a HeaderValue>,
    ) -> PullFuture<'a>;
}

impl PullClient for crate::config_https::HttpsClient {
    fn get<'a>(
        &'a self,
        publisher: &'a Publisher,
        target: &'a PathAndQuery,
        conditional: Option<&'a HeaderValue>,
    ) -> PullFuture<'a> {
        Box::pin(async move {
            use crate::config_https::HttpsError;
            let addr = publisher
                .address()
                .await
                .map_err(|_| TransportError::Network)?;
            let response = self
                .get(
                    addr,
                    publisher.public_key,
                    &publisher.authority,
                    target,
                    conditional,
                )
                .await
                .map_err(|error| match error {
                    HttpsError::Io => TransportError::Network,
                    HttpsError::Timeout => TransportError::Timeout,
                    HttpsError::Tls => TransportError::Pin,
                    HttpsError::InvalidBody => TransportError::InvalidBody,
                    _ => TransportError::Protocol,
                })?;
            Ok(PullResponse {
                peer: response.peer,
                status: response.status,
                headers: response.headers,
                body: response.body,
            })
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullDenial {
    Pin,
    Authority,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PullResult {
    Written(WriteOutcome),
    NotModified,
    Denied(PullDenial),
}

impl PullResult {
    pub fn write_outcome(&self) -> Option<WriteOutcome> {
        match *self {
            Self::Written(outcome) => Some(outcome),
            Self::NotModified | Self::Denied(_) => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PullError {
    WrongRole,
    Transport(TransportError),
    Protocol,
    InvalidErrorBody,
    RemoteConflict,
    RemoteStorage,
    Status(StatusCode),
    Storage { effect_unknown: bool },
    Bundle,
    Rejected,
}

impl fmt::Display for PullError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PullError {}

impl PullError {
    fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Transport(TransportError::Network | TransportError::Timeout)
                | Self::RemoteStorage
                | Self::Storage {
                    effect_unknown: false
                }
        )
    }
}

fn storage(_: sqlx::Error) -> PullError {
    no_effect()
}

fn no_effect() -> PullError {
    PullError::Storage {
        effect_unknown: false,
    }
}

impl From<WriteError> for PullError {
    fn from(error: WriteError) -> Self {
        match error {
            WriteError::Transient(_) => Self::Storage {
                effect_unknown: false,
            },
            WriteError::Rejected(_) => Self::Rejected,
        }
    }
}

fn target(cluster: &str) -> PathAndQuery {
    let mut encoded = String::new();
    for byte in cluster.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").expect("escrita em String");
        }
    }
    format!("{PATH}?cluster={encoded}&namespace=federation&scope_id=")
        .parse()
        .expect("rota canônica da configuração")
}

fn matches_publisher(peer: &Peer, publisher: &Publisher) -> bool {
    peer.node_id == publisher.node_id && peer.public_key == publisher.public_key
}

impl ConfigPlane {
    /// Executa um ciclo. Ciclos concorrentes no mesmo plano não sobrepõem pulls.
    pub async fn poll_cycle(&self, client: &dyn PullClient) -> Result<PullResult, PullError> {
        let publisher = self.config.publisher().ok_or(PullError::WrongRole)?;
        if self.config.policy.role != Role::Receiver {
            return Err(PullError::WrongRole);
        }
        let _one_cycle = self.poll.lock().await;
        let target = target(self.portable.cluster());
        let attempts = self.config.max_attempts.get();
        for attempt in 1..=attempts {
            match self.poll_attempt(client, publisher, &target).await {
                Err(error) if error.retryable() && attempt < attempts => {
                    tokio::time::sleep(self.config.retry_interval).await;
                }
                result => return result,
            }
        }
        unreachable!("o orçamento de tentativas é positivo");
    }

    async fn conditional_hash(&self, deadline: Instant) -> Result<Option<[u8; 32]>, PullError> {
        let mut tx = self.begin().await.map_err(storage)?;
        limit_statement_time(&mut tx, deadline)
            .await
            .map_err(storage)?;
        let mut guard = self
            .portable
            .lock_cluster_guard(&mut tx)
            .await
            .map_err(|_| PullError::Storage {
                effect_unknown: false,
            })?;
        let accepted = self.bundle.accepted_hash_in_tx(&mut guard).await?;
        tx.commit().await.map_err(storage)?;
        Ok(accepted)
    }

    async fn poll_attempt(
        &self,
        client: &dyn PullClient,
        publisher: &Publisher,
        target: &PathAndQuery,
    ) -> Result<PullResult, PullError> {
        let deadline = Instant::now() + self.config.pull_timeout;
        let expected = timeout_at(deadline, self.conditional_hash(deadline))
            .await
            .map_err(|_| no_effect())??;
        let conditional = expected.as_ref().map(etag);
        let response = match timeout_at(
            deadline,
            client.get(publisher, target, conditional.as_ref()),
        )
        .await
        {
            Ok(Ok(reply)) => reply,
            Ok(Err(TransportError::Pin)) => return Ok(PullResult::Denied(PullDenial::Pin)),
            Ok(Err(TransportError::InvalidBody)) => return Err(PullError::InvalidErrorBody),
            Ok(Err(error)) => return Err(PullError::Transport(error)),
            Err(_) => return Err(PullError::Transport(TransportError::Timeout)),
        };
        if !matches_publisher(&response.peer, publisher) {
            return Ok(PullResult::Denied(PullDenial::Pin));
        }
        if response.body.len() > self.config.max_body_bytes.get() {
            return Err(if response.status == StatusCode::SERVICE_UNAVAILABLE {
                PullError::InvalidErrorBody
            } else {
                PullError::Protocol
            });
        }
        match response.status {
            StatusCode::NOT_MODIFIED => {
                let tag =
                    read_etag(&response.headers, header::ETAG).map_err(|()| PullError::Protocol)?;
                if expected.is_none() || tag != expected || !response.body.is_empty() {
                    return Err(PullError::Protocol);
                }
                Ok(PullResult::NotModified)
            }
            StatusCode::OK => {
                if !json_media(&response.headers) {
                    return Err(PullError::Protocol);
                }
                let tag =
                    read_etag(&response.headers, header::ETAG).map_err(|()| PullError::Protocol)?;
                let bundle = authenticate_bundle(&response.body, &self.config.policy.bundle)
                    .map_err(|_| PullError::Bundle)?;
                if tag.as_ref() != Some(bundle.hash()) {
                    return Err(PullError::Protocol);
                }
                self.accept_download(response, deadline).await
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                let error = parse_remote_error(&response.headers, &response.body)
                    .ok_or(PullError::InvalidErrorBody)?;
                if error.retryable {
                    Err(PullError::RemoteStorage)
                } else {
                    Err(PullError::RemoteConflict)
                }
            }
            status => Err(PullError::Status(status)),
        }
    }

    async fn accept_download(
        &self,
        response: PullResponse,
        deadline: Instant,
    ) -> Result<PullResult, PullError> {
        if Instant::now() >= deadline {
            return Err(no_effect());
        }
        let mut tx = timeout_at(deadline, self.begin())
            .await
            .map_err(|_| no_effect())?
            .map_err(storage)?;
        let written = timeout_at(
            deadline,
            self.prepare_download(&mut tx, &response, deadline),
        )
        .await
        .map_err(|_| no_effect())?;
        match written {
            Ok(PullResult::Written(outcome)) => {
                timeout_at(deadline, limit_statement_time(&mut tx, deadline))
                    .await
                    .map_err(|_| no_effect())?
                    .map_err(storage)?;
                // timeout_at pode consultar o futuro interior no instante
                // limite. Não inicie COMMIT se o orçamento já acabou.
                if Instant::now() >= deadline {
                    return Err(no_effect());
                }
                let unknown = || PullError::Storage {
                    effect_unknown: true,
                };
                timeout_at(deadline, self.commit_write(tx, outcome))
                    .await
                    .map_err(|_| unknown())?
                    .map_err(|_| unknown())?;
                Ok(PullResult::Written(outcome))
            }
            other => {
                timeout_at(deadline, tx.rollback())
                    .await
                    .map_err(|_| no_effect())?
                    .map_err(storage)?;
                other
            }
        }
    }

    async fn prepare_download(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        response: &PullResponse,
        deadline: Instant,
    ) -> Result<PullResult, PullError> {
        limit_statement_time(tx, deadline).await.map_err(storage)?;
        let mut guard = self
            .portable
            .lock_cluster_guard(tx)
            .await
            .map_err(|_| no_effect())?;
        let now = self.clock.now_unix();
        let pin = self.config.publisher().ok_or(PullError::WrongRole)?;
        if !matches_publisher(&response.peer, pin) {
            return Ok(PullResult::Denied(PullDenial::Pin));
        }
        // Para pull, o publicador é sempre um par remoto. Mesmo um pin
        // configurado para a própria identidade não substitui a prova.
        let authorized = async {
            self.portable.authorize_local_at(&mut guard, now).await?;
            self.portable
                .authorize_peer_at(
                    &mut guard,
                    &response.peer.node_id,
                    &hex::encode(response.peer.public_key),
                    now,
                )
                .await
        }
        .await;
        match authorized {
            Ok(_) => {}
            Err(PortableJoinError::Storage(_) | PortableJoinError::Encoding(_)) => {
                return Err(PullError::Storage {
                    effect_unknown: false,
                });
            }
            Err(_) => return Ok(PullResult::Denied(PullDenial::Authority)),
        }
        Ok(PullResult::Written(
            self.bundle
                .accept_in_tx(&mut guard, &self.portable, &response.body, now)
                .await?,
        ))
    }

    /// Cancelar o shutdown cancela a tentativa, a espera de retry ou a cadência.
    pub async fn run_polling(&self, client: &dyn PullClient, mut shutdown: watch::Receiver<bool>) {
        if self.config.policy.role != Role::Receiver {
            return;
        }
        loop {
            if *shutdown.borrow() {
                return;
            }
            tokio::select! {
                biased;
                _ = stopped(&mut shutdown) => return,
                result = self.poll_cycle(client) => match result {
                    Err(error) => eprintln!("config_poll outcome={error}"),
                    Ok(PullResult::Denied(reason)) => eprintln!("config_poll outcome=denied reason={reason:?}"),
                    Ok(_) => {}
                }
            }
            tokio::select! {
                biased;
                _ = stopped(&mut shutdown) => return,
                _ = tokio::time::sleep(self.config.poll_interval) => {}
            }
        }
    }
}

async fn stopped(shutdown: &mut watch::Receiver<bool>) {
    let _ = shutdown.wait_for(|stop| *stop).await;
}
