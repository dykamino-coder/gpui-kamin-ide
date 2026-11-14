//! BR-29B: a keyboard focus move scrolls its scroll container so the focused
//! element is visible, like `focus()` in a browser. A mouse focus and later
//! scrolling by the user do not scroll it back.
//!
//! The element records its painted bounds (after the current scroll offset)
//! during the frame in which the focus arrived. The scroll container then
//! applies the smallest offset change on the next frame.

use std::sync::{LazyLock, Mutex};

use gpui::prelude::*;
use gpui::{Bounds, Pixels, Point, ScrollHandle, Window, canvas, div, px};

/// (id waiting to be revealed, its painted bounds once known).
type Pending = (Option<String>, Option<Bounds<Pixels>>);
static REVEAL: LazyLock<Mutex<Pending>> = LazyLock::new(|| Mutex::new((None, None)));

/// Note the focus change of this frame. Only a keyboard move asks for a
/// reveal; a mouse focus or a lost focus drops a pending one.
pub(super) fn focus_changed(visible: bool, focused: Option<&str>) {
    *REVEAL.lock().unwrap() = match focused {
        Some(id) if visible => (Some(id.to_string()), None),
        _ => (None, None),
    };
}

/// A child that records the element's bounds while the reveal is pending.
pub(super) fn probe<E: ParentElement>(el: E, id: &str) -> E {
    if REVEAL.lock().unwrap().0.as_deref() != Some(id) {
        return el;
    }
    let id = id.to_string();
    el.child(
        div().absolute().top_0().left_0().size_full().child(
            canvas(
                move |bounds, window: &mut Window, _| {
                    let mut reveal = REVEAL.lock().unwrap();
                    if reveal.0.as_deref() == Some(id.as_str()) && reveal.1 != Some(bounds) {
                        reveal.1 = Some(bounds);
                        // The container applies the offset on the next frame.
                        window.refresh();
                    }
                },
                |_, _, _, _| {},
            )
            .size_full(),
        ),
    )
}

/// Scroll `handle` so the pending element with an id starting with `prefix`
/// is visible. Call while rendering the scroll container, before its children.
pub fn reveal_in(handle: &ScrollHandle, prefix: &str) {
    let mut reveal = REVEAL.lock().unwrap();
    let (Some(id), Some(item)) = (reveal.0.as_deref(), reveal.1) else {
        return;
    };
    if !id.starts_with(prefix) {
        return;
    }
    let view = handle.bounds();
    if view.size.height <= px(0.) {
        // The container has not been laid out yet; keep the request.
        return;
    }
    if let Some(offset) = revealed_offset(view, item, handle.offset(), handle.max_offset()) {
        handle.set_offset(offset);
    }
    *reveal = (None, None);
}

/// Smallest vertical offset change that brings `item` (painted with `offset`)
/// into `view`, or `None` when it is already fully visible.
pub(crate) fn revealed_offset(
    view: Bounds<Pixels>,
    item: Bounds<Pixels>,
    offset: Point<Pixels>,
    max: Point<Pixels>,
) -> Option<Point<Pixels>> {
    let mut y = offset.y;
    if item.top() < view.top() || item.size.height > view.size.height {
        y += view.top() - item.top();
    } else if item.bottom() > view.bottom() {
        y -= item.bottom() - view.bottom();
    } else {
        return None;
    }
    // The offset is 0 at the top and grows negative down to -max.
    if y < -max.y {
        y = -max.y;
    }
    if y > px(0.) {
        y = px(0.);
    }
    Some(Point { x: offset.x, y })
}
