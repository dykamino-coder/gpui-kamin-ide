//! CSSOM View root requests share the same controller as native input.

use super::Document;
pub use crate::interactive::scroll_animation::ScrollBehavior;
use crate::interactive::scroll_animation::Source;
use gpui::{App, Window, point, px};

impl Document {
    /// Root offset in CSS pixels, down and right positive.
    pub fn root_scroll_offset(&self) -> Option<(f32, f32)> {
        let offset = self.state.root_scroll.as_ref()?.handle.offset();
        Some((-f32::from(offset.x), -f32::from(offset.y)))
    }

    /// Synchronize an active animation before a DOM geometry query (`scrollTop`,
    /// scrollLeft or a bounding rectangle). This does not wait for the next vsync
    /// or build the document. Future JS geometry bindings must use this barrier.
    pub fn current_root_scroll_offset(&self, window: &mut Window, cx: &App) -> Option<(f32, f32)> {
        self.synchronize_scrolling(window, cx);
        self.root_scroll_offset()
    }

    /// Sample every scrollport before a DOM geometry getter, including nested
    /// ancestors that affect bounding rectangles.
    pub fn synchronize_scrolling(&self, window: &mut Window, cx: &App) {
        self.state.scrolling.synchronize(window, cx);
    }

    /// CSSOM View scroll request. `Auto` resolves the root's computed
    /// `scroll-behavior`; `Instant` cancels an active animation. Returns false
    /// when no rendered root scrollport is available or the goal is unchanged.
    /// Future DOM scrollTo/scrollIntoView bindings use this same request path.
    pub fn scroll_to(
        &self,
        x: f32,
        y: f32,
        behavior: ScrollBehavior,
        window: &mut Window,
        cx: &App,
    ) -> bool {
        let Some(root) = self.state.root_scroll.as_ref() else {
            return false;
        };
        let animation = root.animation.borrow();
        let Some(animation) = animation.as_ref() else {
            return false;
        };
        let normalize = |v: f32| if v.is_finite() { v } else { 0. };
        animation.to(
            point(px(-normalize(x)), px(-normalize(y))),
            behavior,
            Source::Programmatic,
            window,
            cx,
        )
    }

    /// Relative CSSOM View scroll request, based on the current scroll position.
    pub fn scroll_by(
        &self,
        x: f32,
        y: f32,
        behavior: ScrollBehavior,
        window: &mut Window,
        cx: &App,
    ) -> bool {
        let Some((left, top)) = self.current_root_scroll_offset(window, cx) else {
            return false;
        };
        let normalize = |v: f32| if v.is_finite() { v } else { 0. };
        self.scroll_to(
            left + normalize(x),
            top + normalize(y),
            behavior,
            window,
            cx,
        )
    }
}
