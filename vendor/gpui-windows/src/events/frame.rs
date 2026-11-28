//! Native draw dispatch and retained reentrant demand.

use super::*;

impl WindowsWindowInner {
    #[inline]
    pub(super) fn draw_window(&self, handle: HWND, draw: NativeDraw) -> Option<isize> {
        let NativeDraw {
            force_render,
            require_presentation,
        } = draw;
        log::debug!(target: "gpui::frame", "native force={} present={} pending={}",
            force_render, require_presentation, self.state.force_render_pending.get());
        let Some(_guard) = self.state.draw_coordinator.try_begin_draw() else {
            log::debug!("deferring re-entrant draw of window {handle:?}");
            if force_render {
                self.state.force_render_pending.set(true);
            }
            self.state
                .presentation_pending
                .set(require_presentation || self.state.presentation_pending.get());
            self.state.frame_demand.request();
            // A posted paced request survives this validation. Native paint
            // must be validated to avoid spinning a nested message pump.
            unsafe { ValidateRect(Some(handle), None).ok().log_err() };
            return Some(0);
        };
        let mut request_frame = self.state.callbacks.request_frame.take()?;

        // A native paint also serves demand already pending before dispatch.
        // Producers running below must retain their demand for the next turn.
        self.state.frame_demand.take();

        self.state.direct_manipulation.update();

        let events = self.state.direct_manipulation.drain_events();
        if !events.is_empty() {
            if let Some(mut func) = self.state.callbacks.input.take() {
                for event in events {
                    func(event);
                }
                self.state.callbacks.input.set(Some(func));
            }
        }

        let force_render = consume_force_render(force_render, &self.state.force_render_pending);
        let require_presentation =
            consume_force_render(require_presentation, &self.state.presentation_pending);
        if force_render {
            // Re-enable drawing after a device loss recovery. The forced render
            // will rebuild the scene with fresh atlas textures.
            self.state.renderer.borrow_mut().mark_drawable();
        }
        let (signal_at, signal_source) = self.state.frame_signal.take().map_or(
            (None, FrameRequestSource::NativeCallback),
            |(at, source)| (Some(at), source),
        );
        request_frame(RequestFrameOptions {
            require_presentation,
            force_render,
            signal_at,
            signal_source,
        });

        self.state.callbacks.request_frame.set(Some(request_frame));
        // A skipped swap-chain present and the finite native resize animation
        // both remain work even when GPUI's retained scene is unchanged.
        if self.state.renderer.borrow().needs_presentation()
            || self
                .state
                .resize_boost_until
                .get()
                .is_some_and(|until| std::time::Instant::now() < until)
        {
            self.state.frame_demand.request();
        }
        self.update_ime_enabled(handle);
        unsafe { ValidateRect(Some(handle), None).ok().log_err() };

        Some(0)
    }
}
