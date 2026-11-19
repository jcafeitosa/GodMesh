use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::{SocketAddr, SocketAddrV4},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use godmesh_federation::{identity::node_id_for, node::LinkIdentity};
use sqlx::Connection;
use tokio::{
    sync::watch,
    time::{Instant, timeout_at},
};

use crate::{
    config_bundle::{BundleService, Fence},
    portable_join::{PortableJoin, PortableJoinError},
    replica_operational::{db::Lease, with_sql_deadline},
};

use super::{
    Direction, Error, Probe, SendCredit, Usage,
    quotas::{OPEN_TIMEOUT, Quotas, Reservation},
    scheduling::{Scheduler, Turn},
};

const CHECK_BUDGET: Duration = Duration::from_millis(100);
const RECHECK_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Debug)]
pub struct Route {
    pub id: String,
    pub requester: LinkIdentity,
    pub destination: LinkIdentity,
    pub service: String,
    pub endpoint: SocketAddr,
}

pub fn normalize_endpoint(addr: SocketAddr) -> Result<SocketAddr, Error> {
    let addr = match addr {
        SocketAddr::V6(v6) if v6.scope_id() != 0 || v6.flowinfo() != 0 => {
            return Err(Error::RouteDenied);
        }
        SocketAddr::V6(v6) => v6
            .ip()
            .to_ipv4_mapped()
            .map_or(addr, |ip| SocketAddr::V4(SocketAddrV4::new(ip, v6.port()))),
        SocketAddr::V4(_) => addr,
    };
    if addr.ip().is_unspecified() || addr.ip().is_multicast() || addr.port() == 0 {
        return Err(Error::RouteDenied);
    }
    Ok(addr)
}

pub struct Registry {
    relay: LinkIdentity,
    routes: HashMap<String, Arc<Route>>,
}

impl Registry {
    /// `reserved` enumera aliases, listeners, shims e outros encaminhamentos locais.
    pub fn new(
        relay: LinkIdentity,
        routes: Vec<Route>,
        reserved: impl IntoIterator<Item = SocketAddr>,
    ) -> Result<Self, Error> {
        if !valid_pin(&relay, &relay.cluster) {
            return Err(Error::RouteDenied);
        }
        let mut endpoints = reserved
            .into_iter()
            .map(normalize_endpoint)
            .collect::<Result<HashSet<_>, _>>()?;
        let mut entries = HashMap::new();
        let mut destinations = HashSet::new();
        for mut route in routes {
            route.endpoint = normalize_endpoint(route.endpoint)?;
            if !valid_route_id(&route.id)
                || route.service.trim().is_empty()
                || !valid_pin(&route.requester, &relay.cluster)
                || !valid_pin(&route.destination, &relay.cluster)
                || route.requester.node_id == relay.node_id
                || route.destination.node_id == relay.node_id
                || route.destination.node_id == route.requester.node_id
                || !endpoints.insert(route.endpoint)
            {
                return Err(Error::RouteDenied);
            }
            destinations.insert(route.destination.node_id.clone());
            if entries.insert(route.id.clone(), Arc::new(route)).is_some() {
                return Err(Error::RouteDenied);
            }
        }
        if entries.is_empty() || destinations.len() > 2 {
            return Err(Error::RouteDenied);
        }
        Ok(Self {
            relay,
            routes: entries,
        })
    }
}

fn valid_pin(pin: &LinkIdentity, cluster: &str) -> bool {
    !cluster.is_empty() && pin.cluster == cluster && pin.node_id == node_id_for(&pin.public_key)
}

fn valid_route_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

pub struct RelayAdmission {
    registry: Registry,
    quotas: Quotas,
    scheduler: Scheduler,
    allowed: Mutex<HashSet<String>>,
    portable: Arc<PortableJoin>,
    m3: Arc<BundleService>,
    clock: Arc<dyn Fn() -> Duration + Send + Sync>,
}

impl RelayAdmission {
    pub fn new(
        registry: Registry,
        portable: Arc<PortableJoin>,
        m3: Arc<BundleService>,
        started_at: Instant,
    ) -> Result<Self, Error> {
        let relay = &registry.relay;
        if relay.cluster != portable.cluster()
            || relay.node_id != portable.identity().node_id()
            || relay.public_key != portable.identity().public_key_bytes()
            || m3.policy().bundle.cluster != relay.cluster
        {
            return Err(Error::RouteDenied);
        }
        Ok(Self {
            allowed: Mutex::new(HashSet::new()),
            registry,
            portable,
            m3,
            clock: Arc::new(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
            }),
            quotas: Quotas::new(started_at),
            scheduler: Scheduler::default(),
        })
    }

    #[cfg(test)]
    pub(super) fn with_clock(mut self, clock: Arc<dyn Fn() -> Duration + Send + Sync>) -> Self {
        self.clock = clock;
        self
    }

    /// ACL explícita da fixture. Revogar fecha imediatamente a instância afetada.
    pub fn set_allowed(&self, route_id: &str, allowed: bool) -> Result<(), Error> {
        let route = self
            .registry
            .routes
            .get(route_id)
            .ok_or(Error::RouteDenied)?;
        let mut acl = self.allowed.lock().unwrap();
        if allowed {
            acl.insert(route_id.to_owned());
        } else {
            acl.remove(route_id);
            self.quotas
                .cancel_route(&route.destination.node_id, route_id);
        }
        Ok(())
    }

    pub fn trust_changes(&self) -> watch::Receiver<u64> {
        self.portable.trust_changes()
    }

    pub fn reserve_probe(&self) -> Result<Probe, Error> {
        self.quotas.reserve_probe()
    }

    pub fn usage(&self, destination: Option<&str>, direction: Direction) -> Usage {
        self.quotas.usage(destination, direction)
    }

    pub async fn open(
        self: &Arc<Self>,
        route_id: &str,
        peer: &LinkIdentity,
        opened_at: Instant,
    ) -> Result<RouteInstance, Error> {
        let route = self
            .registry
            .routes
            .get(route_id)
            .ok_or(Error::RouteDenied)?
            .clone();
        if route.requester != *peer {
            return Err(Error::RouteDenied);
        }
        let open_deadline = opened_at
            .checked_add(OPEN_TIMEOUT)
            .ok_or(Error::RouteUnavailable)?;
        let _opening_turn = timeout_at(open_deadline, self.scheduler.opening())
            .await
            .map_err(|_| Error::RouteUnavailable)?;
        let _turn = timeout_at(open_deadline, self.scheduler.normal())
            .await
            .map_err(|_| Error::RouteUnavailable)?;
        let mut reservation = self
            .authorize(&route, open_deadline, |_, checked_now| {
                self.quotas.reserve_route(
                    &route.destination.node_id,
                    route_id,
                    opened_at,
                    checked_now,
                )
            })
            .await?;
        reservation.publish();
        Ok(RouteInstance {
            admission: self.clone(),
            route,
            open_deadline,
            reservation,
        })
    }

    async fn authorize<T>(
        &self,
        route: &Route,
        deadline: Instant,
        decision: impl FnOnce(Instant, &dyn Fn() -> Result<Instant, Error>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if deadline <= Instant::now() {
            return Err(Error::RouteUnavailable);
        }
        let mut lease = Lease::acquire(self.portable.pool(), deadline)
            .await
            .map_err(|_| Error::RouteUnavailable)?;
        let mut tx = timeout_at(
            deadline,
            lease.begin_with("BEGIN ISOLATION LEVEL READ COMMITTED"),
        )
        .await
        .map_err(|_| Error::RouteUnavailable)?
        .map_err(|_| Error::RouteUnavailable)?;
        let result = timeout_at(
            deadline,
            with_sql_deadline(deadline, async {
                let mut guard = self
                    .portable
                    .lock_cluster_guard(&mut tx)
                    .await
                    .map_err(|_| Error::RouteUnavailable)?;
                let sampled_at = Instant::now();
                let wall = (self.clock)();
                let now = i64::try_from(wall.as_secs()).map_err(|_| Error::AuthorityBlocked)?;
                let mut expiry = self
                    .portable
                    .authorize_local_at(&mut guard, now)
                    .await
                    .map_err(portable_error)?;
                for peer in [&route.requester, &route.destination] {
                    let peer_expiry = self
                        .portable
                        .authorize_peer_at(
                            &mut guard,
                            &peer.node_id,
                            &hex::encode(peer.public_key),
                            now,
                        )
                        .await
                        .map_err(portable_error)?;
                    expiry = expiry.into_iter().chain(peer_expiry).min();
                }
                let Fence::Allowed {
                    recheck_at,
                    expires_at,
                    ..
                } = self
                    .m3
                    .classify_fence_in_tx(&mut guard, 1, now)
                    .await
                    .map_err(|_| Error::RouteUnavailable)?
                else {
                    return Err(Error::AuthorityBlocked);
                };
                let acl = self.allowed.lock().unwrap();
                if !acl.contains(&route.id) {
                    return Err(Error::RouteDenied);
                }
                let expiry = expiry
                    .into_iter()
                    .chain([recheck_at, expires_at])
                    .min()
                    .unwrap();
                let expiry = Duration::from_secs(expiry.try_into().unwrap_or_default());
                let until_expiry = expiry.saturating_sub(wall);
                let checked_now = || {
                    let instant = Instant::now();
                    if instant >= deadline {
                        return Err(Error::RouteUnavailable);
                    }
                    // Uma mudança de validade durante as consultas invalida a
                    // amostra. O mesmo teste vale ao adquirir o lock da quota.
                    if instant.saturating_duration_since(sampled_at) >= until_expiry
                        || (self.clock)() >= expiry
                    {
                        return Err(Error::AuthorityBlocked);
                    }
                    Ok(instant)
                };
                let decided_at = checked_now()?;
                let next_check = (decided_at + RECHECK_INTERVAL)
                    .min(sampled_at + until_expiry.min(RECHECK_INTERVAL));
                decision(next_check, &checked_now)
            }),
        )
        .await
        .unwrap_or(Err(Error::RouteUnavailable));
        if !matches!(timeout_at(deadline, tx.rollback()).await, Ok(Ok(()))) {
            return Err(Error::RouteUnavailable);
        }
        lease.clean();
        result
    }
}

fn portable_error(error: PortableJoinError) -> Error {
    match error {
        PortableJoinError::Storage(_) => Error::RouteUnavailable,
        _ => Error::AuthorityBlocked,
    }
}

pub struct RouteInstance {
    admission: Arc<RelayAdmission>,
    route: Arc<Route>,
    open_deadline: Instant,
    reservation: Reservation,
}

impl RouteInstance {
    pub fn route(&self) -> &Route {
        &self.route
    }

    pub fn open_deadline(&self) -> Instant {
        self.open_deadline
    }

    pub fn cancellation(&self) -> watch::Receiver<bool> {
        self.reservation.cancellation()
    }

    /// O chamador só solicita esta transição depois de verificar o token da stream.
    pub async fn confirm_token(&self) -> Result<Instant, Error> {
        let _opening_turn = self
            .wait_turn_until(self.open_deadline, self.admission.scheduler.opening())
            .await?;
        let _turn = self
            .wait_turn_until(self.open_deadline, self.admission.scheduler.normal())
            .await?;
        self.authorize(self.open_deadline, |next, checked_now| {
            self.reservation.forward(checked_now)?;
            Ok(next)
        })
        .await
    }

    pub fn enqueue(&self, direction: Direction, datagram: Vec<u8>) -> Result<(), Error> {
        self.reservation.enqueue(direction, datagram)
    }

    /// Espera a fila da direção e a disponibilidade da vaga de envio.
    pub async fn wait_for_packet(&self, direction: Direction) -> Result<(), Error> {
        self.reservation.wait_for_packet(direction).await
    }

    pub async fn next_credit(&self, direction: Direction) -> Result<Option<SendCredit>, Error> {
        let mut cancelled = self.cancellation();
        let _turn = tokio::select! {
            biased;
            _ = cancelled.wait_for(|cancelled| *cancelled) => return Err(Error::Cancelled),
            turn = self.admission.scheduler.normal() => turn,
        };
        self.authorize(Instant::now() + CHECK_BUDGET, |_, checked_now| {
            self.reservation.take(direction, checked_now)
        })
        .await
    }

    /// Chamar independentemente das tarefas de I/O, no prazo retornado.
    pub async fn recheck(&self) -> Result<Instant, Error> {
        self.recheck_until(Instant::now() + CHECK_BUDGET).await
    }

    async fn recheck_until(&self, deadline: Instant) -> Result<Instant, Error> {
        let _turn = self
            .wait_turn_until(deadline, self.admission.scheduler.recheck())
            .await?;
        self.authorize(deadline, |next, _| {
            if *self.reservation.cancellation().borrow() {
                return Err(Error::Cancelled);
            }
            Ok(next)
        })
        .await
    }

    async fn wait_turn_until<'a>(
        &'a self,
        deadline: Instant,
        turn: impl Future<Output = Turn<'a>>,
    ) -> Result<Turn<'a>, Error> {
        let mut cancelled = self.cancellation();
        tokio::select! {
            biased;
            _ = cancelled.wait_for(|cancelled| *cancelled) => Err(Error::Cancelled),
            result = timeout_at(deadline, turn) => match result {
                Ok(turn) => Ok(turn),
                Err(_) => {
                    self.cancel();
                    Err(Error::RouteUnavailable)
                }
            },
        }
    }

    /// Executar em tarefa independente de filas e envios da instância.
    pub async fn run_rechecks(
        &self,
        mut next: Instant,
        mut m3_changes: Option<watch::Receiver<u64>>,
    ) -> Result<(), Error> {
        let mut cancelled = self.cancellation();
        let mut trust = Some(self.admission.trust_changes());
        while !*cancelled.borrow_and_update() {
            let triggered_at = tokio::select! {
                biased;
                _ = cancelled.changed() => return Ok(()),
                _ = changed(&mut trust) => Instant::now().min(next),
                _ = changed(&mut m3_changes) => Instant::now().min(next),
                _ = tokio::time::sleep_until(next) => next,
            };
            next = self.recheck_until(triggered_at + CHECK_BUDGET).await?;
        }
        Ok(())
    }

    pub fn cancel(&self) {
        self.reservation.cancel();
    }

    /// Invocar após encerrar e juntar as tarefas da instância.
    pub fn recollect(self) {
        self.reservation.recollect();
    }

    async fn authorize<T>(
        &self,
        deadline: Instant,
        decision: impl FnOnce(Instant, &dyn Fn() -> Result<Instant, Error>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if *self.reservation.cancellation().borrow() {
            return Err(Error::Cancelled);
        }
        let result = self
            .admission
            .authorize(&self.route, deadline, decision)
            .await;
        if matches!(
            result,
            Err(Error::AuthorityBlocked | Error::RouteDenied | Error::RouteUnavailable)
        ) {
            self.cancel();
        }
        result
    }
}

async fn changed(receiver: &mut Option<watch::Receiver<u64>>) {
    if let Some(current) = receiver {
        if current.changed().await.is_ok() {
            return;
        }
        *receiver = None;
    }
    std::future::pending().await
}
