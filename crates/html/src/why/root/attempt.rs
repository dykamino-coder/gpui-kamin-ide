//! Exclusive nesting and callback gaps for one observed root attempt.

use super::{Attempt, Data, Phase, Scope};
use serde_json::json;
use std::{
    cell::{RefCell, RefMut},
    time::Instant,
};

impl Data {
    fn charge(&mut self, now: Instant) {
        if let (Some(phase), Some(changed)) = (self.active, self.changed) {
            self.exclusive[phase as usize] += now.duration_since(changed);
        }
        self.changed = Some(now);
    }
}

impl Attempt {
    fn observed(&self) -> bool {
        let Some(stored) = &self.0 else {
            return false;
        };
        let cell: &RefCell<Data> = stored;
        if !crate::why::enabled() {
            cell.borrow_mut().live = false;
            return false;
        }
        cell.borrow().live
    }

    pub(crate) fn enter(&self, phase: Phase) -> Scope {
        if !self.observed() {
            return Scope(None);
        }
        self.enter_at(phase, Instant::now())
    }

    pub(super) fn enter_at(&self, phase: Phase, now: Instant) -> Scope {
        let Some(stored) = &self.0 else {
            return Scope(None);
        };
        let cell: &RefCell<Data> = stored;
        let mut data = cell.borrow_mut();
        if !data.live {
            return Scope(None);
        }
        data.charge(now);
        let previous = data.active.replace(phase);
        data.calls[phase as usize] += 1;
        Scope(Some((self.clone(), phase, previous, now)))
    }

    pub(crate) fn requested(&self, scale: f32) {
        if !self.observed() {
            return;
        }
        if let Some(stored) = &self.0 {
            let cell: &RefCell<Data> = stored;
            let mut data: RefMut<'_, Data> = cell.borrow_mut();
            if data.live {
                data.identity["scale_factor"] = json!(scale);
                data.gap = Some(Instant::now());
            }
        }
    }

    pub(crate) fn prepaint(&self) -> Scope {
        if !self.observed() {
            return Scope(None);
        }
        let now = Instant::now();
        if let Some(stored) = &self.0 {
            let cell: &RefCell<Data> = stored;
            let mut data = cell.borrow_mut();
            if data.live
                && let Some(start) = data.gap.take()
            {
                let elapsed = now.duration_since(start);
                let i = Phase::HostLayout as usize;
                data.inclusive[i] += elapsed;
                data.exclusive[i] += elapsed;
                data.calls[i] += 1;
            }
        }
        self.enter_at(Phase::Prepaint, now)
    }

    pub(crate) fn painted(&self) {
        if !self.observed() {
            return;
        }
        if let Some(stored) = &self.0 {
            let cell: &RefCell<Data> = stored;
            cell.borrow_mut().painted = true;
        }
    }
}

impl Scope {
    pub(super) fn finish_at(&mut self, now: Instant) {
        if let Some((attempt, phase, previous, start)) = self.0.take() {
            let cell: &RefCell<Data> = attempt.0.as_deref().unwrap();
            let mut data = cell.borrow_mut();
            if data.live {
                data.charge(now);
                data.inclusive[phase as usize] += now.duration_since(start);
                data.active = previous;
            }
        }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if self
            .0
            .as_ref()
            .is_some_and(|(attempt, ..)| attempt.observed())
        {
            self.finish_at(Instant::now());
        }
    }
}
