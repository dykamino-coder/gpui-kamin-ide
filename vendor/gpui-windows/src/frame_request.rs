//! Спрос на кадр Windows, приём сигналов readback и защита общего UI-потока
//! от вложенной отрисовки.

use std::{cell::Cell, rc::Rc};
use gpui_util::ResultExt;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{RedrawWindow, RDW_INVALIDATE};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
use crate::{WM_GPUI_FRAME_READBACK, WindowsWindowInner};

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
        // `on_request_frame` callback itself, which no-ops in that case.
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

impl DrawCoordinator {
    // Запрос из отрисовки обслуживает vsync: немедленный повтор обходил бы
    // ограничение частоты анимаций и создавал цикл при throttling.
    fn wake_if_idle(&self, wake: impl FnOnce()) {
        if !self.drawing.get() {
            wake();
        }
    }
}

impl WindowsWindowInner {
    pub(crate) fn frame_waker(self: &Rc<Self>) -> Rc<dyn Fn()> {
        let window = Rc::downgrade(self);
        Rc::new(move || {
            if let Some(window) = window.upgrade() {
                window.state.draw_coordinator.wake_if_idle(|| {
                    // Без UPDATENOW: WM_PAINT объединяет запросы и не входит в App
                    // синхронно из invalidate_view/set_dirty.
                    unsafe { RedrawWindow(Some(window.hwnd), None, None, RDW_INVALIDATE) }
                        .ok().log_err();
                });
            }
        })
    }
}

impl WindowsWindowInner {
    /// Сигнал GPU о копии кадра: забрать пиксели вне отрисовки. Если рендерер
    /// сейчас занят (вложенная обработка сообщений), сигнал откладывается в
    /// очередь окна; новый кадр сам заберёт копию прежде своей.
    pub(crate) fn handle_frame_readback(&self, hwnd: HWND, wparam: WPARAM) -> Option<isize> {
        match self.state.renderer.try_borrow_mut() {
            Ok(mut renderer) => renderer.finish_frame_readback(Some(wparam.0)),
            Err(_) => unsafe {
                PostMessageW(Some(hwnd), WM_GPUI_FRAME_READBACK, wparam, LPARAM(0))
                    .log_err();
            },
        }
        Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_demand_wakes_without_entering_a_draw() {
        let coordinator = DrawCoordinator::new();
        let wakes = Cell::new(0);
        coordinator.wake_if_idle(|| wakes.set(wakes.get() + 1));
        assert_eq!(wakes.get(), 1);
        assert!(coordinator.try_begin_draw().is_some());
    }

    #[test]
    fn nested_demand_waits_for_vsync_and_guard_restores_idle_waking() {
        let coordinator = DrawCoordinator::new();
        let wakes = Cell::new(0);
        let guard = coordinator.try_begin_draw().unwrap();
        assert!(coordinator.try_begin_draw().is_none());
        coordinator.wake_if_idle(|| wakes.set(wakes.get() + 1));
        assert_eq!(wakes.get(), 0);
        drop(guard);
        coordinator.wake_if_idle(|| wakes.set(wakes.get() + 1));
        assert_eq!(wakes.get(), 1);
        assert!(coordinator.try_begin_draw().is_some());
    }
}
