//! Weak vsync driver, external cancellation and trajectory preparation barrier.

use super::*;

impl Scroll {
    pub(super) fn schedule(&self, window: &mut Window) {
        if self.0.pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(&self.0);
        window.on_next_frame(move |window, cx| {
            let Some(state) = weak.upgrade() else { return };
            let scroll = Self(state);
            scroll.0.pending.set(false);
            scroll.synchronize(window, cx);
        });
    }

    /// Geometry getters sample the same absolute clock between display frames.
    pub(crate) fn synchronize(&self, window: &mut Window, cx: &App) {
        self.synchronize_at(self.now(cx), window, cx);
    }

    pub(super) fn synchronize_at(&self, now: Duration, window: &mut Window, cx: &App) {
        let Some(mut motion) = self.0.motion.borrow_mut().take() else {
            return;
        };
        // A drag, script assignment or scroll anchor may supersede the request.
        if self.0.last.get() != self.0.handle.offset() {
            return;
        }
        if self.0.tracker.preparing() {
            self.0.waiting.set(self.0.waiting.get().or(Some(now)));
            self.0.motion.replace(Some(motion));
            self.schedule(window);
            window.refresh();
            return;
        }
        if let Some(since) = self.0.waiting.take() {
            motion.delay(now.saturating_sub(since));
        }
        let sample = motion.sample(now);
        let target = self.clamp(pixels(motion.target));
        let done = sample.done || cx.reduce_motion() || target != pixels(motion.target);
        let offset = if done {
            target
        } else {
            self.clamp(pixels(sample.position))
        };
        if offset != self.0.last.get() {
            self.0.tracker.frame();
        }
        self.0.handle.set_offset(offset);
        self.0.last.set(offset);
        if !done {
            self.0.motion.replace(Some(motion));
            self.schedule(window);
        }
        window.refresh();
    }
}
