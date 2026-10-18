//! Transparent wrapper that reports the layout node of a watched box.
//!
//! The pagination solver needs the laid-out border boxes of the break
//! candidates inside a copy. `render::element` wraps an element whose id is
//! in the watch set (only while a paginated copy is being built); the wrapper
//! returns the inner layout node as its own, so taffy sees the same tree.

use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

thread_local! {
    static WATCH: RefCell<Option<HashSet<u64>>> = const { RefCell::new(None) };
    static SEEN: RefCell<HashMap<u64, LayoutId>> = RefCell::new(HashMap::new());
}

/// Build with the watch set `ids` active.
pub(crate) fn building<T>(ids: &[u64], f: impl FnOnce() -> T) -> T {
    let prev = WATCH.with(|w| w.borrow_mut().replace(ids.iter().copied().collect()));
    let out = f();
    WATCH.with(|w| *w.borrow_mut() = prev);
    out
}

/// Wrap `el` when `id` is watched (identity otherwise).
pub(crate) fn wrap(id: u64, el: AnyElement) -> AnyElement {
    let on = WATCH.with(|w| w.borrow().as_ref().is_some_and(|s| s.contains(&id)));
    if on {
        Watch { id, inner: el }.hooked()
    } else {
        el
    }
}

/// Forget recorded nodes before a measuring layout.
pub(crate) fn reset() {
    SEEN.with(|s| s.borrow_mut().clear());
}

/// Layout node recorded for `id` by the last layout.
pub(crate) fn seen(id: u64) -> Option<LayoutId> {
    SEEN.with(|s| s.borrow().get(&id).copied())
}

struct Watch {
    id: u64,
    inner: AnyElement,
}

impl Hooks for Watch {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let lid = self.inner.request_layout(window, cx);
        SEEN.with(|s| s.borrow_mut().insert(self.id, lid));
        (lid, ())
    }

    fn prepaint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        self.inner.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(window, cx);
    }
}
