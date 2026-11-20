//! One native scroll owner for a full-page document, shared with its viewport adapter.

use gpui::{KeyDownEvent, ScrollHandle, point, px};
use std::cell::Cell;

#[derive(Default)]
pub(crate) struct RootScroll {
    pub(crate) handle: ScrollHandle,
    pub(crate) axes: Cell<[bool; 2]>,
}

impl RootScroll {
    pub(crate) fn key(&self, event: &KeyDownEvent) -> bool {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
            return false;
        }
        let handle = &self.handle;
        let before = handle.offset();
        let max = handle.max_offset();
        let page = (f32::from(handle.bounds().size.height) - 40.).max(1.);
        let mut next = before;
        match event.keystroke.key.as_str() {
            "up" => next.y += px(40.),
            "down" => next.y -= px(40.),
            "left" => next.x += px(40.),
            "right" => next.x -= px(40.),
            "pageup" => next.y += px(page),
            "pagedown" => next.y -= px(page),
            "space" => next.y += px(if modifiers.shift { page } else { -page }),
            "home" => next.y = px(0.),
            "end" => next.y = -max.y,
            _ => return false,
        }
        let [horizontal, vertical] = self.axes.get();
        next = point(
            if horizontal {
                next.x.clamp(-max.x, px(0.))
            } else {
                before.x
            },
            if vertical {
                next.y.clamp(-max.y, px(0.))
            } else {
                before.y
            },
        );
        if next == before {
            return false;
        }
        handle.set_offset(next);
        true
    }
}
