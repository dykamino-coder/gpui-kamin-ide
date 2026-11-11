//! KaminIDE patch: work run once the app has drawn no frame for a while.
//!
//! A host registers one callback ([`App::on_idle`]) — the binary's allocator
//! returning freed memory to the system, as Blink's `PurgeMemory` on idle
//! does for PartitionAlloc. The platform keeps the idle period without heap
//! memory: the first frame after an idle period arms one timer, frames drawn
//! while it waits only move its deadline, so the callback runs once, `delay`
//! after the last frame, and an idle app has no timer. A platform without
//! such a timer never calls the callback.

use super::App;
use std::time::Duration;

impl App {
    /// KaminIDE patch: run `callback` once no window has drawn a frame for
    /// `delay`; again after each later period of frames. The first
    /// registration holds.
    pub fn on_idle(&mut self, delay: Duration, callback: fn()) {
        self.platform.on_idle(delay, callback);
    }

    /// A window drew a frame: the idle period starts again.
    pub(crate) fn frame_drawn(&self) {
        self.platform.frame_drawn();
    }
}
