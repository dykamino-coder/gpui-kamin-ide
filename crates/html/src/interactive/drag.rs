//! Завершение перетаскивания мышью для состояния интерактивного элемента.

use gpui::{MouseUpEvent, Window};
use std::{cell::Cell, rc::Rc};

pub(super) fn release_on_mouse_up<S: Copy + 'static>(
    state: Rc<Cell<S>>,
    dragging: fn(&mut S) -> &mut bool,
    window: &mut Window,
) {
    window.on_mouse_event(move |_e: &MouseUpEvent, phase, _window, _cx| {
        if !phase.bubble() {
            return;
        }
        let mut value = state.get();
        let active = dragging(&mut value);
        if *active {
            *active = false;
            state.set(value);
        }
    });
}
