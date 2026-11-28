//! A demand latch for the platform worker; idle has no timeout or timer.

use parking_lot::{Condvar, Mutex};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(crate) struct FramePacer {
    work: Mutex<bool>,
    changed: Condvar,
}

impl FramePacer {
    pub(crate) fn request(&self) {
        *self.work.lock() = true;
        self.changed.notify_one();
    }

    pub(crate) fn wait(&self, stop: &AtomicBool) -> bool {
        let mut work = self.work.lock();
        while !*work && !stop.load(Ordering::Acquire) {
            self.changed.wait(&mut work);
        }
        if stop.load(Ordering::Acquire) {
            return false;
        }
        *work = false;
        true
    }

    pub(crate) fn stop(&self, stop: &AtomicBool) {
        // Serialize with the wait predicate: notify cannot slip before park.
        let _work = self.work.lock();
        stop.store(true, Ordering::Release);
        self.changed.notify_all();
    }
}

pub(crate) struct FrameDemand {
    pending: AtomicBool,
    posted: AtomicBool,
    pacer: Arc<FramePacer>,
}

impl FrameDemand {
    pub(crate) fn new(pacer: Arc<FramePacer>) -> Self {
        Self {
            pending: AtomicBool::new(false),
            posted: AtomicBool::new(false),
            pacer,
        }
    }

    pub(crate) fn request(&self) {
        self.pending.store(true, Ordering::Release);
        self.pacer.request();
    }

    pub(crate) fn take(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn claim_post(&self) -> bool {
        !self.posted.swap(true, Ordering::AcqRel)
    }

    pub(crate) fn handled(&self) {
        self.posted.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_before_park_coalesce_but_requests_after_consume_survive() {
        let pacer = Arc::new(FramePacer::default());
        let demand = FrameDemand::new(pacer.clone());
        let stop = AtomicBool::new(false);
        demand.request();
        demand.request();
        assert!(pacer.wait(&stop));
        assert!(demand.take());
        assert!(!demand.take());
        assert!(demand.claim_post());
        demand.request();
        assert!(pacer.wait(&stop));
        assert!(demand.take());
        assert!(!demand.claim_post());
        // Dispatch releases the posted slot before drawing.
        demand.handled();
        demand.request();
        assert!(pacer.wait(&stop));
        assert!(demand.take());
        assert!(demand.claim_post());
    }

    #[test]
    fn stop_notifies_a_parked_worker_and_does_not_stop_a_new_attachment() {
        let pacer = Arc::new(FramePacer::default());
        let stop = Arc::new(AtomicBool::new(false));
        let worker = {
            let pacer = pacer.clone();
            let stop = stop.clone();
            std::thread::spawn(move || assert!(!pacer.wait(&stop)))
        };
        pacer.stop(&stop);
        worker.join().unwrap();
        let next_stop = AtomicBool::new(false);
        pacer.request();
        assert!(pacer.wait(&next_stop));
        assert!(stop.load(Ordering::Acquire));
    }

    #[test]
    fn windows_keep_independent_demand_and_clearing_a_post_does_not_clear_new_work() {
        let pacer = Arc::new(FramePacer::default());
        let a = FrameDemand::new(pacer.clone());
        let b = FrameDemand::new(pacer);
        a.request();
        b.request();
        assert!(a.take());
        assert!(a.claim_post());
        a.request();
        a.handled();
        assert!(a.take());
        assert!(b.take());
        assert!(a.claim_post());
        assert!(b.claim_post());
    }
}
