//! Transparent wrapper that reports the layout node of a watched box.
//!
//! The pagination solver needs the laid-out border boxes of the break
//! candidates inside a copy. A paginated copy is built with a `Watched`
//! recorder in its `RenderOpts` (`FragCx::watch`); `render::element` wraps an
//! element whose id the recorder watches, and the wrapper returns the inner
//! layout node as its own (taffy sees the same tree) after recording it.

use crate::hooks::{Hooks, IntoHooked};
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Break candidates of one paginated child and the layout nodes its last
/// measuring layout gave them. Owned by the child's `Paginate`, shared with
/// the wrappers of its rebuilt copies.
#[derive(Default)]
pub(crate) struct Watched {
    pub(crate) flow: Rc<crate::layout::fragment::flow::FlowMeasurements>,
    ids: HashSet<u64>,
    seen: RefCell<HashMap<u64, LayoutId>>,
}

impl Watched {
    pub(crate) fn new(ids: &[u64]) -> Rc<Self> {
        Rc::new(Watched {
            ids: ids.iter().copied().collect(),
            seen: RefCell::default(),
            flow: Rc::default(),
        })
    }

    pub(crate) fn for_lines() -> Rc<Self> {
        Rc::new(Self {
            flow: Rc::new(crate::layout::fragment::flow::FlowMeasurements::for_lines()),
            ..Self::default()
        })
    }

    /// Forget recorded nodes before a measuring layout.
    pub(crate) fn reset(&self) {
        self.seen.borrow_mut().clear();
        self.flow.reset();
    }

    /// Layout node recorded for `id` by the last layout.
    pub(crate) fn seen(&self, id: u64) -> Option<LayoutId> {
        self.seen.borrow().get(&id).copied()
    }
}

impl std::fmt::Debug for Watched {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watched")
            .field("ids", &self.ids.len())
            .finish()
    }
}

/// Wrap `el` when the build's recorder watches `id` (identity otherwise).
pub(crate) fn wrap(id: u64, el: AnyElement, opts: &RenderOpts) -> AnyElement {
    match &opts.frag.watch {
        Some(rec) if rec.ids.contains(&id) => Watch {
            id,
            inner: el,
            rec: rec.clone(),
        }
        .hooked(),
        _ => el,
    }
}

struct Watch {
    id: u64,
    inner: AnyElement,
    rec: Rc<Watched>,
}

impl Hooks for Watch {
    type Layout = ();
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let lid = self.inner.request_layout(window, cx);
        self.rec.seen.borrow_mut().insert(self.id, lid);
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
