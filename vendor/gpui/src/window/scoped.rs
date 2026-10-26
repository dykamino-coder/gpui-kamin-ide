//! Восстановление состояния окна при обычном выходе и раскрутке стека.

use super::Window;

impl Window {
    pub(super) fn with_restore<S, R>(
        &mut self,
        mut state: S,
        restore: fn(&mut Window, &mut S),
        f: impl FnOnce(&mut Window) -> R,
    ) -> (R, S) {
        struct Restore<'a, S> {
            window: &'a mut Window,
            state: &'a mut S,
            restore: fn(&mut Window, &mut S),
        }
        impl<S> Drop for Restore<'_, S> {
            fn drop(&mut self) {
                (self.restore)(self.window, self.state);
                if std::thread::panicking() { self.window.paint_pass.invalidate(); }
            }
        }
        let result = {
            let guard = Restore { window: self, state: &mut state, restore };
            f(guard.window)
        };
        (result, state)
    }
}
