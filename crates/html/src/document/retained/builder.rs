//! The frame build an element runs itself when no replay fits (`Retained`).

use crate::document::state::DocState;
use crate::dom::Node;
use crate::layout::reuse::FlowReuse;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use std::cell::Cell;
use std::rc::Rc;

/// The frame build the element runs itself when the replay did not fit.
pub(super) struct Builder {
    pub(super) state: Rc<DocState>,
    pub(super) nodes: Rc<Vec<Node>>,
    /// The number of tree `nodes` (`Document::tree`).
    pub(super) tree: u64,
    pub(super) opts: crate::RenderOpts,
    pub(super) store: Rc<crate::layout::reuse::Store>,
    /// The build counters before the frame's first build: a second build (after
    /// a stand-in refusal) starts from them, as the first did.
    pub(super) counters: Cell<Option<crate::layout::reuse::Counters>>,
    /// No resource changed since the previous frame: the window's frame tree
    /// may keep measured leaves by their layout keys (gpui::Window::with_layout_keys).
    pub(super) layout_keys: bool,
    pub(super) phases: crate::why::root::Attempt,
}

impl Builder {
    /// The viewport box with the document tree: an initial containing block of
    /// exact size, the UA white canvas and the window text of the host options.
    /// Root-flow blocks are recorded for the next frames and, with `allow`,
    /// fitting ones are replaced by their earlier records (`layout::reuse`).
    /// `drawn`: a build inside prepaint, the frame already begun (`Store::draw`).
    /// `hover`: the hover the window shows, which replays check
    /// (`layout::reuse::ReplayRecord::hovers_hold`).
    pub(super) fn build(
        &self,
        allow: bool,
        drawn: bool,
        hover: gpui::HoverView,
    ) -> (AnyElement, Option<Rc<FlowReuse>>) {
        let _phase = self.phases.enter(crate::why::root::Phase::Build);
        let (w, h) = self.opts.viewport;
        let again = self.counters.get().is_some();
        match self.counters.get() {
            Some(counters) => counters.write(&self.opts.cx),
            None => self
                .counters
                .set(Some(crate::layout::reuse::Counters::read(&self.opts.cx))),
        }
        let flow = FlowReuse::new(self.store.clone(), &self.nodes, allow, drawn, again, hover);
        let mut opts = self.opts.clone();
        opts.cx.contracts.reuse = flow.clone();
        let tree = div()
            .w(px(w))
            .h(px(h))
            .bg(rgb(0xffffff))
            .text_size(self.opts.text.font_size)
            .font_family(self.opts.text.font_family.clone())
            .children(super::super::scope::render(
                &self.state,
                (&self.nodes, self.tree),
                &opts,
            ))
            .into_any_element();
        (tree, flow)
    }
}
