//! Retained Windows frame demand and protection against nested UI draws.

use std::{cell::Cell, rc::Rc};

mod pacer;
use crate::WindowsWindowInner;
use gpui_util::ResultExt;
pub(crate) use pacer::{FrameDemand, FramePacer};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

/// Coordinates window draws on the UI thread. Owned by the platform and
/// shared with every window (like `WindowsPlatformState::cursor_visible`),
/// because the coordination is inherently cross-window: while window A is
/// drawing, a re-entrant paint request for window B must be deferred.
pub(crate) struct DrawCoordinator {
    /// Whether some window is currently inside `draw_window`. Win32 can
    /// re-enter the window procedure while a draw is in progress (e.g.
    /// cross-thread `SendMessage` dispatch during message pumping, or modal
    /// message loops entered by COM calls), and drawing re-entrantly would
    /// nest GPUI draws. Nested draws are wasted work whose output is
    /// immediately redrawn, so we defer them instead.
    drawing: Cell<bool>,
}

impl DrawCoordinator {
    pub(crate) fn new() -> Self {
        Self {
            drawing: Cell::new(false),
        }
    }

    pub(crate) fn try_begin_draw(&self) -> Option<DrawWindowGuard<'_>> {
        // This only covers the `draw_window` span (which extends past the GPUI
        // draw, through presentation and IME updates). Requests that arrive
        // re-entrantly during GPUI-initiated draws (e.g. key dispatch or
        // opening a window draws synchronously) are deferred by GPUI's
        // `on_request_frame` callback itself, which retains a retry demand.
        if self.drawing.get() {
            None
        } else {
            self.drawing.set(true);
            Some(DrawWindowGuard { coordinator: self })
        }
    }
}

pub(crate) struct DrawWindowGuard<'a> {
    coordinator: &'a DrawCoordinator,
}

impl Drop for DrawWindowGuard<'_> {
    fn drop(&mut self) {
        self.coordinator.drawing.set(false);
    }
}

impl WindowsWindowInner {
    pub(crate) fn frame_waker(self: &Rc<Self>) -> Rc<dyn Fn()> {
        let window = Rc::downgrade(self);
        Rc::new(move || {
            if let Some(window) = window.upgrade() {
                window.state.frame_demand.request();
            }
        })
    }
}

/// Posts a slot already claimed by the paced worker. Unlike an update region,
/// this message survives ValidateRect at the end of an in-progress draw.
pub(crate) fn post_frame_request(hwnd: HWND, demand: &FrameDemand) {
    if unsafe {
        PostMessageW(
            Some(hwnd),
            crate::events::WM_GPUI_FRAME_REQUEST,
            WPARAM(0),
            LPARAM(0),
        )
    }
    .log_err()
    .is_none()
    {
        demand.handled();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_draws_defer_and_demand_survives_until_the_guard_unwinds() {
        let coordinator = DrawCoordinator::new();
        let demand = FrameDemand::new(std::sync::Arc::new(FramePacer::default()));
        let guard = coordinator.try_begin_draw().unwrap();
        assert!(coordinator.try_begin_draw().is_none());
        demand.request();
        drop(guard);
        assert!(coordinator.try_begin_draw().is_some());
        assert!(demand.take());
        assert!(!demand.take());
    }
}
