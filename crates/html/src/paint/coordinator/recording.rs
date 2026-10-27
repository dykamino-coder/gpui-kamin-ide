//! Завершение CSS-заданий внутри записи до закрытия её механического scope.

use super::*;
use gpui::PaintRecording;

pub(super) fn recording() -> PaintRecording {
    PaintRecording {
        begin: Rc::new(|window| match current(window) {
            Some(pass) => Box::new(pass.open(window)),
            None => Box::new(()),
        }),
        finish: Rc::new(|guard, window, cx| {
            if let Some(guard) = guard.downcast_mut::<FrameGuard>() {
                guard.pass.drain(guard.depth, window, cx);
            }
        }),
    }
}
