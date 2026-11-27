//! Consume wheel input only when this CSS scrollport actually moves.

use gpui::{DispatchPhase, Hitbox, ScrollHandle, ScrollWheelEvent, Window, point, px};
use std::cell::Cell;

/// `wheel`: the axes a wheel scrolls (`overflow: hidden` ones scroll only by script).
pub(super) fn listen(hitbox: Hitbox, handle: ScrollHandle, wheel: [bool; 2], window: &mut Window) {
    let before = Cell::new(handle.offset());
    window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
        if !hitbox.should_handle_scroll(window) {
            return;
        }
        if phase == DispatchPhase::Capture {
            before.set(handle.offset());
            return;
        }
        // GPUI applies the native delta before this adapter's bubble callback,
        // where the content is hit: inside the scrollport's clip. Over the rest
        // of the scrollport (a bar's gutter) nothing moved it, and the wheel
        // scrolls it all the same, as a browser scrolls the box whose
        // scrollbar the pointer is over. At a boundary the clamp undoes it.
        let mut current = handle.offset();
        if current == before.get() {
            let delta = event.delta.pixel_delta(window.line_height());
            current += point(
                if wheel[0] { delta.x } else { px(0.) },
                if wheel[1] { delta.y } else { px(0.) },
            );
        }
        // Clamp here so a boundary event can chain to the parent in this event.
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
