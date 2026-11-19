use std::{net::SocketAddr, sync::Arc, time::Duration};

use godmesh_federation::{
    link::LinkState,
    node::{DialAttempt, DialCancellation, DialTarget, FederationNode},
    peer::PeerRecord,
};
use sqlx::PgPool;
use tokio::{sync::watch, task::JoinHandle};

use super::{Candidate, Driver, Pending, maintain, refresh};

pub(super) struct Node(pub(super) Arc<FederationNode>);

impl Driver for Node {
    type Attempt = DialAttempt;

    fn publish(&self, records: &[PeerRecord]) {
        self.0.registry().replace(records.iter().cloned());
    }

    fn eligible_now(&self, candidate: &Candidate) -> bool {
        candidate.cluster == self.0.config().cluster
            && self
                .0
                .registry()
                .record(&candidate.id)
                .and_then(|record| Candidate::from_record(&record))
                .as_ref()
                == Some(candidate)
            && !self
                .0
                .links()
                .snapshot(&candidate.id)
                .is_some_and(|snapshot| snapshot.state == LinkState::Ready)
    }

    fn start(&self, peer: &Candidate, addr: SocketAddr) -> Option<Self::Attempt> {
        let target = DialTarget {
            node_id: peer.id.clone(),
            public_key: peer.key,
            addr,
        };
        match self.0.start_dial(target) {
            Ok(attempt) => Some(attempt),
            Err(error) => {
                eprintln!(
                    "godmesh-server: discagem para {} por {addr} recusada: {error}",
                    peer.id
                );
                None
            }
        }
    }
}

impl Pending for DialAttempt {
    fn cancel(&mut self) {
        if let DialCancellation::AlreadyPromoted(connection) = DialAttempt::cancel(self) {
            connection.invalidate();
        }
    }

    async fn finish(&mut self) -> bool {
        self.wait().await.is_ok()
    }
}

/// Mantém o registro e discagens até o sinal de shutdown. A tarefa somente
/// termina após recolher as tentativas e o refresh SQL que possuía.
pub fn spawn(
    pool: PgPool,
    node: Arc<FederationNode>,
    cadence: Duration,
    stop: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let node = Node(node);
        maintain(&node, node.0.identity().node_id(), cadence, stop, |stop| {
            refresh::read(&pool, stop)
        })
        .await;
    })
}
