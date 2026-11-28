//! One native scroll owner for a full-page document, shared with its viewport adapter.

use gpui::{KeyDownEvent, KeyUpEvent, ScrollHandle, point, px};
use std::cell::Cell;

#[derive(Default)]
pub(crate) struct RootScroll {
    pub(crate) handle: ScrollHandle,
    pub(crate) animation: std::cell::RefCell<Option<crate::interactive::scroll_animation::Scroll>>,
    pub(crate) axes: Cell<[bool; 2]>,
    /// Inputs that only moved the offset (a scroll key): the document frame
    /// does not count them as interaction (`WindowKey::capture`) and
    /// replays them through its scroll nodes.
    pub(crate) scroll_inputs: Cell<u64>,
    /// The key the last scroll key press scrolled with, until its release.
    pressed: Cell<Option<String>>,
    /// The document's frame element draws the bars (`RootBars`): set by it
    /// before its tree prepaints.
    pub(crate) hosted: Cell<bool>,
    /// The bars of the last built frame.
    pub(crate) bars:
        std::cell::RefCell<Option<std::rc::Rc<crate::interactive::scroll_area::RootBars>>>,
}

impl RootScroll {
    pub(crate) fn key(
        &self,
        event: &KeyDownEvent,
        window: &mut gpui::Window,
        cx: &gpui::App,
    ) -> bool {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
            return false;
        }
        let handle = &self.handle;
        let animation = self.animation.borrow();
        let Some(animation) = animation.as_ref() else {
            return false;
        };
        let before = animation.target();
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
        use crate::interactive::scroll_animation::{ScrollBehavior, Source};
        animation.to(next, ScrollBehavior::Smooth, Source::Keyboard, window, cx);
        self.scroll_inputs
            .set(self.scroll_inputs.get().wrapping_add(1));
        self.pressed.set(Some(event.keystroke.key.clone()));
        true
    }

    /// The release of the key `key` scrolled with is one more input that
    /// only moved the offset.
    pub(crate) fn released(&self, event: &KeyUpEvent) {
        let pressed = self.pressed.take();
        if pressed.as_ref() == Some(&event.keystroke.key) {
            self.scroll_inputs
                .set(self.scroll_inputs.get().wrapping_add(1));
        } else {
            self.pressed.set(pressed);
        }
    }
}
