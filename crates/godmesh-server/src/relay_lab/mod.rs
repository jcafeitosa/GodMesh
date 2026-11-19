//! Admissão e contabilidade do relay exclusivo da fixture M5.
#[cfg(test)]
mod tests;

mod admission;
mod quotas;
mod scheduling;

pub use admission::{Registry, RelayAdmission, Route, RouteInstance, normalize_endpoint};
pub use quotas::{Direction, MAX_CONTROL_BYTES, MAX_DATAGRAM_BYTES, Probe, SendCredit, Usage};

#[cfg(test)]
use quotas::{IO_TIMEOUT, Quotas, WINDOW};
#[cfg(test)]
use tokio::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    RouteDenied,
    AuthorityBlocked,
    RouteUnavailable,
    QuotaExceeded,
    QueueFull,
    RateExceeded,
    DatagramSize,
    Cancelled,
}
