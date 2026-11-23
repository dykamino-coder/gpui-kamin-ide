//! Missing line geometry explained to DevTools subscribers only.
//! The map never affects reuse. Two generations bound memory and preserve
//! paragraph preparation that happened before paint began.

use super::Store;
use crate::dom::BoxId;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Refusals {
    containers: HashMap<BoxId, (u64, &'static str)>,
    previous: HashMap<BoxId, (u64, &'static str)>,
}

impl Store {
    pub(super) fn begin_fragment_diagnostics(&self) {
        if !crate::why::enabled() {
            return;
        }
        let mut why = self.fragment_diagnostics.borrow_mut();
        why.previous = std::mem::take(&mut why.containers);
    }

    pub(crate) fn fragment_refused(&self, container: BoxId, reason: &'static str) {
        if !crate::why::enabled() {
            return;
        }
        let mut why = self.fragment_diagnostics.borrow_mut();
        let stamp = self.frame_epoch();
        why.containers.insert(container, (stamp, reason));
    }

    pub(crate) fn fragment_refusal(&self, container: BoxId) -> String {
        let why = self.fragment_diagnostics.borrow();
        let (stamp, reason) = why
            .containers
            .get(&container)
            .or_else(|| why.previous.get(&container))
            .copied()
            .unwrap_or((0, "no paragraph capture in two draws"));
        format!(
            "container={container:?} capture={reason} capture_frame={stamp} frame_held={} containers={} previous={}",
            self.fragments_hold(stamp),
            why.containers.len(),
            why.previous.len(),
        )
    }
}
