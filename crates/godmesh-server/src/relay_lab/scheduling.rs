use std::sync::Mutex;

use tokio::sync::{Mutex as Fifo, Notify};

/// Serializa transações de abertura, crédito e rechecagem antes de adquirir uma Lease.
#[derive(Default)]
pub(super) struct Scheduler {
    normal: Fifo<()>,
    opening: Fifo<()>,
    state: Mutex<State>,
    changed: Notify,
}

#[derive(Default)]
struct State {
    active: bool,
    waiting_rechecks: usize,
}

impl Scheduler {
    pub(super) async fn opening(&self) -> OpeningTurn<'_> {
        OpeningTurn {
            _guard: self.opening.lock().await,
        }
    }

    pub(super) async fn normal(&self) -> Turn<'_> {
        let _fifo = self.normal.lock().await;
        self.enter(false).await
    }

    pub(super) async fn recheck(&self) -> Turn<'_> {
        self.state.lock().unwrap().waiting_rechecks += 1;
        let _priority = PendingRecheck(self);
        self.enter(true).await
    }

    async fn enter(&self, recheck: bool) -> Turn<'_> {
        loop {
            let changed = self.changed.notified();
            {
                let mut state = self.state.lock().unwrap();
                if !state.active && (recheck || state.waiting_rechecks == 0) {
                    state.active = true;
                    return Turn(self);
                }
            }
            changed.await;
        }
    }
}

struct PendingRecheck<'a>(&'a Scheduler);

impl Drop for PendingRecheck<'_> {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().waiting_rechecks -= 1;
        self.0.changed.notify_waiters();
    }
}

pub(super) struct OpeningTurn<'a> {
    _guard: tokio::sync::MutexGuard<'a, ()>,
}

pub(super) struct Turn<'a>(&'a Scheduler);

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().active = false;
        self.0.changed.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::{Pin, pin},
        task::{Context, Poll, Waker},
    };

    use super::Scheduler;

    fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }

    #[tokio::test]
    async fn credits_enter_fifo_with_only_one_active() {
        let scheduler = Scheduler::default();
        let current = scheduler.normal().await;
        let mut first = pin!(scheduler.normal());
        let mut second = pin!(scheduler.normal());
        assert!(poll(first.as_mut()).is_pending());
        assert!(poll(second.as_mut()).is_pending());

        drop(current);
        assert!(poll(second.as_mut()).is_pending());
        let Poll::Ready(first) = poll(first.as_mut()) else {
            panic!("first queued credit must enter first");
        };
        assert!(poll(second.as_mut()).is_pending());
        drop(first);
        assert!(poll(second.as_mut()).is_ready());
    }

    #[tokio::test]
    async fn waiting_recheck_goes_before_queued_credit() {
        let scheduler = Scheduler::default();
        let current = scheduler.normal().await;
        let mut credit = pin!(scheduler.normal());
        let mut recheck = pin!(scheduler.recheck());
        assert!(poll(credit.as_mut()).is_pending());
        assert!(poll(recheck.as_mut()).is_pending());

        drop(current);
        assert!(poll(credit.as_mut()).is_pending());
        let Poll::Ready(recheck) = poll(recheck.as_mut()) else {
            panic!("recheck must enter ahead of queued credit");
        };
        assert!(poll(credit.as_mut()).is_pending());
        drop(recheck);
        assert!(poll(credit.as_mut()).is_ready());
    }

    #[tokio::test]
    async fn dropping_pending_recheck_releases_its_priority() {
        let scheduler = Scheduler::default();
        let current = scheduler.normal().await;
        let mut credit = pin!(scheduler.normal());
        let mut recheck = Box::pin(scheduler.recheck());
        assert!(poll(credit.as_mut()).is_pending());
        assert!(poll(recheck.as_mut()).is_pending());

        drop(recheck);
        drop(current);
        assert!(poll(credit.as_mut()).is_ready());
    }
}
