//! Supervisão das discagens de saída. O registro continua sendo a autoridade
//! para agendar e o transporte decide se a conexão pode promover.
use std::{
    collections::{HashMap, VecDeque},
    future::{Future, pending},
    net::SocketAddr,
    time::Duration,
};

use godmesh_federation::peer::PeerRecord;
use tokio::{sync::watch, time::MissedTickBehavior};

mod refresh;
mod transport;

pub use transport::spawn;
#[cfg(test)]
mod tests;

#[derive(Clone, PartialEq, Eq)]
struct Candidate {
    id: String,
    peer_id: String,
    cluster: String,
    epoch: u64,
    key: [u8; 32],
    endpoints: Vec<String>,
}

impl Candidate {
    fn from_record(record: &PeerRecord) -> Option<Self> {
        if !record.is_authorized() {
            return None;
        }
        let mut key = [0; 32];
        hex::decode_to_slice(&record.public_key, &mut key).ok()?;
        Some(Self {
            id: record.node_id.clone(),
            peer_id: record.peer_id.clone(),
            cluster: record.cluster.clone(),
            epoch: record.epoch,
            key,
            endpoints: record.endpoints.clone(),
        })
    }
}

fn parse_endpoint(endpoint: &str) -> Option<SocketAddr> {
    endpoint.trim().strip_prefix("quic://")?.parse().ok()
}

fn stopping(stop: &watch::Receiver<bool>) -> bool {
    *stop.borrow() || stop.has_changed().is_err()
}

trait Pending {
    /// Cancela a tentativa ou invalida somente a conexão que ela promoveu.
    fn cancel(&mut self);

    /// true indica promoção. Deve ser seguro repollar após perder um select.
    fn finish(&mut self) -> impl Future<Output = bool> + Send;
}

trait Driver {
    type Attempt: Pending;
    fn publish(&self, records: &[PeerRecord]);
    fn eligible_now(&self, candidate: &Candidate) -> bool;
    fn start(&self, peer: &Candidate, addr: SocketAddr) -> Option<Self::Attempt>;
}

struct Slot<A> {
    peer: Candidate,
    next_endpoint: usize,
    attempt: Option<A>,
    cancelled: bool,
}

struct Scheduler<'a, D: Driver> {
    driver: &'a D,
    local: &'a str,
    current: HashMap<String, Candidate>,
    fresh: bool,
    queue: VecDeque<String>,
    retry: VecDeque<String>,
    slots: [Option<Slot<D::Attempt>>; 2],
}

impl<'a, D: Driver> Scheduler<'a, D> {
    fn new(driver: &'a D, local: &'a str) -> Self {
        Self {
            driver,
            local,
            current: HashMap::new(),
            fresh: false,
            queue: VecDeque::new(),
            retry: VecDeque::new(),
            slots: [None, None],
        }
    }

    fn refreshed(&mut self, records: Vec<PeerRecord>) {
        self.driver.publish(&records);
        self.fresh = true;
        self.current.clear();
        let mut ordered = Vec::new();
        for record in &records {
            if record.node_id == self.local {
                continue;
            }
            let Some(peer) = Candidate::from_record(record) else {
                continue;
            };
            if !peer
                .endpoints
                .iter()
                .any(|entry| parse_endpoint(entry).is_some())
            {
                continue;
            }
            ordered.push(peer.id.clone());
            self.current.insert(peer.id.clone(), peer);
        }
        self.queue.retain(|id| {
            self.current
                .get(id)
                .is_some_and(|peer| self.driver.eligible_now(peer))
        });
        for slot in self.slots.iter_mut().flatten() {
            if self.current.get(&slot.peer.id) != Some(&slot.peer) {
                Self::cancel_slot(slot);
            }
        }
        for id in std::mem::take(&mut self.retry).into_iter().chain(ordered) {
            self.enqueue(&id);
        }
    }

    fn enqueue(&mut self, id: &str) {
        if self.fresh
            && self
                .current
                .get(id)
                .is_some_and(|peer| self.driver.eligible_now(peer))
            && !self.queue.iter().any(|queued| queued == id)
            && !self.slots.iter().flatten().any(|slot| slot.peer.id == id)
        {
            self.queue.push_back(id.to_owned());
        }
    }

    fn dispatch(&mut self, stop: &watch::Receiver<bool>) {
        if !self.fresh || stopping(stop) {
            return;
        }
        for slot in &mut self.slots {
            if slot.as_ref().is_some_and(|slot| slot.attempt.is_none())
                && !Self::advance(self.driver, &self.current, slot.as_mut().unwrap(), stop)
            {
                let expired = slot.take().unwrap();
                if !expired.cancelled && expired.next_endpoint == expired.peer.endpoints.len() {
                    self.retry.push_back(expired.peer.id);
                }
            }
            while slot.is_none() && !stopping(stop) {
                let Some(id) = self.queue.pop_front() else {
                    break;
                };
                let Some(peer) = self.current.get(&id) else {
                    continue;
                };
                let mut new = Slot {
                    peer: peer.clone(),
                    next_endpoint: 0,
                    attempt: None,
                    cancelled: false,
                };
                if Self::advance(self.driver, &self.current, &mut new, stop) {
                    *slot = Some(new);
                } else if new.next_endpoint == new.peer.endpoints.len() {
                    self.retry.push_back(new.peer.id);
                }
            }
        }
    }

    fn advance(
        driver: &D,
        current: &HashMap<String, Candidate>,
        slot: &mut Slot<D::Attempt>,
        stop: &watch::Receiver<bool>,
    ) -> bool {
        while !slot.cancelled
            && !stopping(stop)
            && current.get(&slot.peer.id) == Some(&slot.peer)
            && driver.eligible_now(&slot.peer)
        {
            let Some(endpoint) = slot.peer.endpoints.get(slot.next_endpoint) else {
                break;
            };
            slot.next_endpoint += 1;
            let Some(addr) = parse_endpoint(endpoint) else {
                continue;
            };
            if let Some(attempt) = driver.start(&slot.peer, addr) {
                slot.attempt = Some(attempt);
                return true;
            }
        }
        false
    }

    async fn reap_one(&mut self) {
        let [first, second] = &mut self.slots;
        let (index, promoted) = tokio::select! {
            result = Self::finish(first) => (0,result),
            result = Self::finish(second) => (1,result),
        };
        let slot = self.slots[index]
            .as_mut()
            .expect("somente tentativa em voo conclui");
        slot.attempt = None;
        if promoted || slot.cancelled {
            let slot = self.slots[index].take().unwrap();
            if slot.cancelled && self.current.get(&slot.peer.id) != Some(&slot.peer) {
                self.enqueue(&slot.peer.id);
            }
        } else if slot.next_endpoint == slot.peer.endpoints.len() {
            // Repetições esperam a próxima leitura autorizada. Conservam a
            // ordem de conclusão e entram atrás de quem já aguardava.
            let slot = self.slots[index].take().unwrap();
            self.retry.push_back(slot.peer.id);
        }
    }

    async fn finish(slot: &mut Option<Slot<D::Attempt>>) -> bool {
        match slot.as_mut().and_then(|slot| slot.attempt.as_mut()) {
            Some(attempt) => attempt.finish().await,
            None => pending().await,
        }
    }

    fn cancel_slot(slot: &mut Slot<D::Attempt>) {
        if !slot.cancelled {
            slot.cancelled = true;
            if let Some(attempt) = &mut slot.attempt {
                attempt.cancel();
            }
        }
    }

    async fn shutdown(mut self) {
        for slot in self.slots.iter_mut().flatten() {
            Self::cancel_slot(slot);
        }
        for slot in self.slots.iter_mut().flatten() {
            if let Some(attempt) = &mut slot.attempt {
                attempt.finish().await;
            }
        }
    }
}

/// Um refresh em voo, supervisionado separadamente das duas discagens.
async fn maintain<D, F, E>(
    driver: &D,
    local: &str,
    cadence: Duration,
    mut stop: watch::Receiver<bool>,
    refresh_records: impl Fn(watch::Receiver<bool>) -> F,
) where
    D: Driver,
    F: Future<Output = Result<Vec<PeerRecord>, E>>,
    E: std::fmt::Debug,
{
    let mut ticker = tokio::time::interval(cadence);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut scheduler = Scheduler::new(driver, local);
    let mut refresh: Option<std::pin::Pin<Box<F>>> = None;
    while !stopping(&stop) {
        scheduler.dispatch(&stop);
        tokio::select! {
            biased;
            () = async { let _ = stop.wait_for(|stop| *stop).await; } => break,
            result = async {
                match &mut refresh {
                    Some(reading) => reading.as_mut().await,
                    None => pending().await,
                }
            } => {
                refresh = None;
                if stopping(&stop) { break; }
                match result {
                    Ok(records) => scheduler.refreshed(records),
                    Err(error) => {
                        scheduler.fresh = false;
                        eprintln!("godmesh-server: recarga da allowlist de pares falhou: {error:?}");
                    }
                }
            }
            _ = ticker.tick() => {
                if refresh.is_none() {
                    scheduler.fresh = false;
                    refresh = Some(Box::pin(refresh_records(stop.clone())));
                }
            }
            () = scheduler.reap_one() => {}
        }
    }
    let cleanup = async move {
        if let Some(in_flight) = refresh {
            let _ = in_flight.await;
        }
    };
    tokio::join!(cleanup, scheduler.shutdown());
}
