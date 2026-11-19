//! M3-B2: plano HTTPS de configuração e pull. O runtime instala os adaptadores.

mod api;
mod pull;
mod settings;

pub(crate) use api::transport_refusal;

use std::sync::Arc;

use godmesh_federation::NodeIdentity;
use sqlx::{Postgres, Transaction};
use tokio::sync::{Mutex, watch};

use crate::{
    admission::{Admission, AdmissionConfig},
    auth::{Authorization, Clock, SystemClock},
    config_bundle::{BundleService, WriteOutcome},
    portable_join::{ClusterGuard, PortableJoin, PortableJoinError},
};

pub use pull::{
    PullClient, PullDenial, PullError, PullFuture, PullResponse, PullResult, TransportError,
};
pub use settings::{ConfigError, ConfigPlaneConfig, Publisher};

pub use crate::config_https::PeerIdentity as Peer;

pub struct ConfigPlane {
    config: ConfigPlaneConfig,
    bundle: Arc<BundleService>,
    portable: Arc<PortableJoin>,
    identity: Arc<NodeIdentity>,
    authorization: Arc<Authorization>,
    admission: Admission,
    clock: Arc<dyn Clock>,
    changes: watch::Sender<u64>,
    poll: Mutex<()>,
}

impl ConfigPlane {
    pub fn new(
        config: ConfigPlaneConfig,
        bundle: Arc<BundleService>,
        portable: Arc<PortableJoin>,
        identity: Arc<NodeIdentity>,
        authorization: Arc<Authorization>,
        admission: AdmissionConfig,
    ) -> Result<Self, ConfigError> {
        let policy = bundle.policy();
        let mut supplied = config.policy.bundle.clone();
        supplied
            .voters
            .sort_unstable_by(|a, b| a.node_id.cmp(&b.node_id));
        if policy.role != config.policy.role
            || policy.profile != config.policy.profile
            || policy.bundle != supplied
            || policy.software != config.policy.software
            || policy.bundle.cluster != portable.cluster()
            || portable
                .local_manifest()
                .map_err(|_| ConfigError("identidade local"))?
                .node_id
                != identity.node_id()
        {
            return Err(ConfigError("política ou identidade divergente"));
        }
        Ok(Self {
            config,
            bundle,
            portable,
            identity,
            authorization,
            admission: Admission::new(admission),
            clock: Arc::new(SystemClock),
            changes: watch::channel(0).0,
            poll: Mutex::new(()),
        })
    }

    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn state(&self) -> &Arc<BundleService> {
        &self.bundle
    }

    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    async fn begin(&self) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
        self.portable
            .pool()
            .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
            .await
    }

    async fn authorize_peer(
        &self,
        guard: &mut ClusterGuard<'_, '_>,
        peer: &Peer,
        now: i64,
    ) -> Result<(), PortableJoinError> {
        self.portable.authorize_local_at(guard, now).await?;
        if peer.node_id == self.identity.node_id()
            && peer.public_key == self.identity.public_key_bytes()
        {
            return Ok(());
        }
        self.portable
            .authorize_peer_at(guard, &peer.node_id, &hex::encode(peer.public_key), now)
            .await?;
        Ok(())
    }

    async fn commit_write(
        &self,
        tx: Transaction<'static, Postgres>,
        outcome: WriteOutcome,
    ) -> Result<(), sqlx::Error> {
        if !matches!(
            outcome,
            WriteOutcome::Accepted
                | WriteOutcome::ConflictRecorded
                | WriteOutcome::IncompatibleRecorded
        ) {
            return tx.commit().await;
        }
        let changes = self.changes.clone();
        // O cancelamento do cliente durante COMMIT não deve suprimir o sinal.
        tokio::spawn(async move {
            tx.commit().await?;
            changes.send_modify(|value| *value = value.wrapping_add(1));
            Ok(())
        })
        .await
        .map_err(|error| sqlx::Error::Protocol(error.to_string()))?
    }
}
