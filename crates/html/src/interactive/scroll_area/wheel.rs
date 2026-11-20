//! Consume wheel input only when this CSS scrollport actually moves.

use gpui::{DispatchPhase, Hitbox, ScrollHandle, ScrollWheelEvent, Window, point, px};
use std::cell::Cell;

pub(super) fn listen(hitbox: Hitbox, handle: ScrollHandle, window: &mut Window) {
    let before = Cell::new(handle.offset());
    window.on_mouse_event(move |_: &ScrollWheelEvent, phase, window, cx| {
        if !hitbox.should_handle_scroll(window) {
            return;
        }
        if phase == DispatchPhase::Capture {
            before.set(handle.offset());
            return;
        }
        // GPUI applies the native delta before this adapter's bubble callback.
        // Clamp here so a boundary event can chain to the parent in this event.
        let current = handle.offset();
        let max = handle.max_offset();
        let next = point(
            current.x.clamp(-max.x, px(0.)),
            current.y.clamp(-max.y, px(0.)),
        );
        handle.set_offset(next);
        if next != before.get() {
            cx.stop_propagation();
        }
    });
}
