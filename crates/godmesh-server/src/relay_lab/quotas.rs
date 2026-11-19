use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    sync::{Notify, watch},
    time::Instant,
};

use super::Error;

pub const MAX_DATAGRAM_BYTES: usize = 1400;
pub const MAX_CONTROL_BYTES: usize = 512;
pub(super) const WINDOW: Duration = Duration::from_secs(1);
pub(super) const OPEN_TIMEOUT: Duration = Duration::from_secs(2);
pub(super) const IO_TIMEOUT: Duration = Duration::from_millis(100);
const PREFIX: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    ToDestination,
    FromDestination,
}

impl Direction {
    fn index(self) -> usize {
        match self {
            Self::ToDestination => 0,
            Self::FromDestination => 1,
        }
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Usage {
    pub debited_frames: u64,
    pub debited_bytes: u64,
    pub queued_frames: usize,
    pub queued_bytes: usize,
    pub queue_drops: u64,
    pub rate_drops: u64,
    pub size_drops: u64,
    pub cancelled_frames: u64,
}

impl Usage {
    fn debit(&mut self, bytes: usize) {
        self.debited_frames += 1;
        self.debited_bytes += bytes as u64;
    }

    fn queued(&mut self, bytes: usize) {
        self.queued_frames += 1;
        self.queued_bytes += bytes;
    }

    fn dequeued(&mut self, bytes: usize) {
        self.queued_frames -= 1;
        self.queued_bytes -= bytes;
    }
}

#[derive(Default)]
struct Budget {
    window: u64,
    frames: usize,
    bytes: usize,
}

impl Budget {
    fn permits(&mut self, window: u64, bytes: usize, frames_max: usize, bytes_max: usize) -> bool {
        if self.window != window {
            *self = Self {
                window,
                ..Self::default()
            };
        }
        self.frames < frames_max && self.bytes + bytes <= bytes_max
    }

    fn debit(&mut self, bytes: usize) {
        self.frames += 1;
        self.bytes += bytes;
    }
}

#[derive(Default)]
struct Lane {
    queue: VecDeque<Vec<u8>>,
    in_flight: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    AwaitingToken,
    Forwarding,
    Cancelled,
}

struct Slot {
    id: u64,
    route_id: String,
    phase: Phase,
    open_deadline: Instant,
    cancelled: watch::Sender<bool>,
    ready: Arc<[Notify; 2]>,
    lanes: [Lane; 2],
    recollected: bool,
}

#[derive(Default)]
struct Destination {
    slot: Option<Slot>,
    usage: [Usage; 2],
    budget: Budget,
}

#[derive(Default)]
struct State {
    next_id: u64,
    active: usize,
    destinations: HashMap<String, Destination>,
    aggregate: [Usage; 2],
    budget: Budget,
    probe_reserved: bool,
    probe_queue: Option<Vec<u8>>,
    probe_budget: Budget,
}

#[derive(Clone)]
pub(super) struct Quotas {
    start: Instant,
    state: Arc<Mutex<State>>,
}

impl Quotas {
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            state: Arc::default(),
        }
    }

    #[cfg(test)]
    pub fn reserve(&self, destination: &str, opened_at: Instant) -> Result<Reservation, Error> {
        self.reserve_route(destination, destination, opened_at, || Ok(opened_at))
    }

    pub fn reserve_route(
        &self,
        destination: &str,
        route_id: &str,
        opened_at: Instant,
        checked_now: impl FnOnce() -> Result<Instant, Error>,
    ) -> Result<Reservation, Error> {
        let deadline = opened_at
            .checked_add(OPEN_TIMEOUT)
            .ok_or(Error::RouteUnavailable)?;
        let mut state = self.state.lock().unwrap();
        checked_now()?;
        if state.active >= 2
            || state
                .destinations
                .get(destination)
                .is_some_and(|d| d.slot.is_some())
        {
            return Err(Error::QuotaExceeded);
        }
        state.next_id = state
            .next_id
            .checked_add(1)
            .ok_or(Error::RouteUnavailable)?;
        let id = state.next_id;
        let (cancelled, receiver) = watch::channel(false);
        let ready = Arc::<[Notify; 2]>::default();
        state
            .destinations
            .entry(destination.to_owned())
            .or_default()
            .slot = Some(Slot {
            id,
            route_id: route_id.to_owned(),
            phase: Phase::AwaitingToken,
            open_deadline: deadline,
            cancelled,
            ready: ready.clone(),
            lanes: Default::default(),
            recollected: false,
        });
        state.active += 1;
        Ok(Reservation {
            quotas: self.clone(),
            destination: destination.to_owned(),
            id,
            receiver,
            ready,
            published: false,
        })
    }

    pub fn cancel_route(&self, destination: &str, route_id: &str) {
        let mut state = self.state.lock().unwrap();
        let State {
            destinations,
            aggregate,
            ..
        } = &mut *state;
        if let Some(destination) = destinations.get_mut(destination)
            && let Some(slot) = destination.slot.as_mut()
            && slot.route_id == route_id
        {
            cancel_slot(slot, &mut destination.usage, aggregate);
        }
    }

    pub fn reserve_probe(&self) -> Result<Probe, Error> {
        let mut state = self.state.lock().unwrap();
        if state.probe_reserved {
            return Err(Error::QuotaExceeded);
        }
        state.probe_reserved = true;
        Ok(Probe(self.clone()))
    }

    pub fn usage(&self, destination: Option<&str>, direction: Direction) -> Usage {
        let state = self.state.lock().unwrap();
        let i = direction.index();
        match destination {
            Some(destination) => state
                .destinations
                .get(destination)
                .map_or_else(Usage::default, |d| d.usage[i]),
            None => state.aggregate[i],
        }
    }

    #[cfg(test)]
    pub fn snapshot(&self, destination: &str) -> Usage {
        let a = self.usage(Some(destination), Direction::ToDestination);
        let b = self.usage(Some(destination), Direction::FromDestination);
        Usage {
            debited_frames: a.debited_frames + b.debited_frames,
            debited_bytes: a.debited_bytes + b.debited_bytes,
            ..Usage::default()
        }
    }

    fn window(&self, now: Instant) -> u64 {
        now.saturating_duration_since(self.start).as_secs() / WINDOW.as_secs()
    }
}

pub(super) struct Reservation {
    quotas: Quotas,
    destination: String,
    id: u64,
    receiver: watch::Receiver<bool>,
    ready: Arc<[Notify; 2]>,
    published: bool,
}

impl Reservation {
    pub fn publish(&mut self) {
        self.published = true;
    }

    pub fn cancellation(&self) -> watch::Receiver<bool> {
        self.receiver.clone()
    }

    pub fn forward(
        &self,
        checked_now: impl FnOnce() -> Result<Instant, Error>,
    ) -> Result<(), Error> {
        self.with_slot(|slot, _, _, _| {
            if slot.phase != Phase::AwaitingToken || checked_now()? >= slot.open_deadline {
                return Err(Error::RouteUnavailable);
            }
            slot.phase = Phase::Forwarding;
            Ok(())
        })
    }

    pub fn enqueue(&self, direction: Direction, datagram: Vec<u8>) -> Result<(), Error> {
        self.with_slot(|slot, destination, aggregate, _| {
            let i = direction.index();
            if slot.phase != Phase::Forwarding {
                return Err(Error::RouteUnavailable);
            }
            if datagram.is_empty() || datagram.len() > MAX_DATAGRAM_BYTES {
                destination.usage[i].size_drops += 1;
                aggregate[i].size_drops += 1;
                return Err(Error::DatagramSize);
            }
            let size = datagram.len() + PREFIX;
            let usage = &mut destination.usage[i];
            let global = &mut aggregate[i];
            if usage.queued_frames >= 4
                || usage.queued_bytes + size > 4800
                || global.queued_frames >= 8
                || global.queued_bytes + size > 9600
            {
                usage.queue_drops += 1;
                global.queue_drops += 1;
                return Err(Error::QueueFull);
            }
            usage.queued(size);
            global.queued(size);
            slot.lanes[i].queue.push_back(datagram);
            slot.ready[i].notify_one();
            Ok(())
        })
    }

    pub async fn wait_for_packet(&self, direction: Direction) -> Result<(), Error> {
        let index = direction.index();
        let mut cancelled = self.cancellation();
        loop {
            let ready = self.with_slot(|slot, _, _, _| {
                if slot.phase != Phase::Forwarding {
                    return Err(Error::RouteUnavailable);
                }
                let lane = &slot.lanes[index];
                Ok(!lane.in_flight && !lane.queue.is_empty())
            })?;
            if ready {
                return Ok(());
            }
            tokio::select! {
                _ = cancelled.wait_for(|cancelled| *cancelled) => return Err(Error::Cancelled),
                _ = self.ready[index].notified() => {},
            }
        }
    }

    pub fn take(
        &self,
        direction: Direction,
        checked_now: impl FnOnce() -> Result<Instant, Error>,
    ) -> Result<Option<SendCredit>, Error> {
        self.with_slot(|slot, destination, aggregate, budget| {
            let i = direction.index();
            if slot.phase != Phase::Forwarding {
                return Err(Error::RouteUnavailable);
            }
            let lane = &mut slot.lanes[i];
            if lane.in_flight {
                return Ok(None);
            }
            let now = checked_now()?;
            let Some(datagram) = lane.queue.pop_front() else {
                return Ok(None);
            };
            let bytes = datagram.len() + PREFIX;
            let usage = &mut destination.usage[i];
            let global = &mut aggregate[i];
            usage.dequeued(bytes);
            global.dequeued(bytes);
            let window = self.quotas.window(now);
            if !destination.budget.permits(window, bytes, 32, 36_000)
                || !budget.permits(window, bytes, 64, 72_000)
            {
                usage.rate_drops += 1;
                global.rate_drops += 1;
                return Err(Error::RateExceeded);
            }
            destination.budget.debit(bytes);
            budget.debit(bytes);
            usage.debit(bytes);
            global.debit(bytes);
            lane.in_flight = true;
            Ok(Some(SendCredit {
                datagram,
                deadline: now + IO_TIMEOUT,
                quotas: self.quotas.clone(),
                destination: self.destination.clone(),
                id: self.id,
                direction,
            }))
        })
    }

    pub fn cancel(&self) {
        let _ = self.with_slot(|slot, destination, aggregate, _| {
            cancel_slot(slot, &mut destination.usage, aggregate);
            Ok(())
        });
    }

    pub fn recollect(self) {
        self.release_recollected();
    }

    fn release_recollected(&self) {
        self.cancel();
        let mut state = self.quotas.state.lock().unwrap();
        if let Some(slot) = state
            .destinations
            .get_mut(&self.destination)
            .and_then(|d| d.slot.as_mut())
            .filter(|s| s.id == self.id)
        {
            slot.recollected = true;
            release(&mut state, &self.destination, self.id);
        }
    }

    fn with_slot<T>(
        &self,
        op: impl FnOnce(&mut Slot, &mut Destination, &mut [Usage; 2], &mut Budget) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut state = self.quotas.state.lock().unwrap();
        let State {
            destinations,
            aggregate,
            budget,
            ..
        } = &mut *state;
        let destination = destinations
            .get_mut(&self.destination)
            .ok_or(Error::Cancelled)?;
        let mut slot = destination.slot.take().ok_or(Error::Cancelled)?;
        let result = if slot.id == self.id && slot.phase != Phase::Cancelled {
            op(&mut slot, destination, aggregate, budget)
        } else {
            Err(Error::Cancelled)
        };
        destination.slot = Some(slot);
        result
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.published {
            self.cancel();
        } else {
            self.release_recollected();
        }
    }
}

fn cancel_slot(slot: &mut Slot, usage: &mut [Usage; 2], aggregate: &mut [Usage; 2]) {
    slot.phase = Phase::Cancelled;
    for (i, lane) in slot.lanes.iter_mut().enumerate() {
        for datagram in lane.queue.drain(..) {
            usage[i].cancelled_frames += 1;
            aggregate[i].cancelled_frames += 1;
            usage[i].dequeued(datagram.len() + PREFIX);
            aggregate[i].dequeued(datagram.len() + PREFIX);
        }
    }
    slot.cancelled.send_replace(true);
}

fn release(state: &mut State, destination: &str, id: u64) {
    let entry = state
        .destinations
        .get_mut(destination)
        .expect("reserved destination");
    if entry.slot.as_ref().is_some_and(|slot| {
        slot.id == id && slot.recollected && slot.lanes.iter().all(|lane| !lane.in_flight)
    }) {
        entry.slot = None;
        state.active -= 1;
    }
}

pub struct SendCredit {
    datagram: Vec<u8>,
    deadline: Instant,
    quotas: Quotas,
    destination: String,
    id: u64,
    direction: Direction,
}

impl SendCredit {
    pub fn datagram(&self) -> &[u8] {
        &self.datagram
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }
}

impl Drop for SendCredit {
    fn drop(&mut self) {
        let mut state = self.quotas.state.lock().unwrap();
        if let Some(slot) = state
            .destinations
            .get_mut(&self.destination)
            .and_then(|d| d.slot.as_mut())
            .filter(|s| s.id == self.id)
        {
            let index = self.direction.index();
            slot.lanes[index].in_flight = false;
            slot.ready[index].notify_one();
            release(&mut state, &self.destination, self.id);
        }
    }
}

pub struct Probe(Quotas);

impl Probe {
    pub fn enqueue(&self, item: Vec<u8>) -> Result<(), Error> {
        if item.is_empty() || item.len() > 64 {
            return Err(Error::DatagramSize);
        }
        let mut state = self.0.state.lock().unwrap();
        if state.probe_queue.is_some() {
            return Err(Error::QueueFull);
        }
        state.probe_queue = Some(item);
        Ok(())
    }

    pub fn take(&self, now: Instant) -> Result<Option<Vec<u8>>, Error> {
        let mut state = self.0.state.lock().unwrap();
        let Some(item) = state.probe_queue.take() else {
            return Ok(None);
        };
        if !state
            .probe_budget
            .permits(self.0.window(now), item.len(), 1, 64)
        {
            return Err(Error::RateExceeded);
        }
        state.probe_budget.debit(item.len());
        Ok(Some(item))
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.probe_queue = None;
        state.probe_reserved = false;
    }
}
