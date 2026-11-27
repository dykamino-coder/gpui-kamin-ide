//! Canonical retained-root construction, shared by preparation and draw.

use super::Builder;
use crate::layout::reuse::FlowReuse;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};
use std::rc::Rc;

impl Builder {
    /// Build the viewport root, recording reusable blocks for later frames.
    pub(super) fn build(&self, allow: bool, drawn: bool) -> (AnyElement, Option<Rc<FlowReuse>>) {
        let _phase = self.phases.enter(crate::why::root::Phase::Build);
        let (w, h) = self.opts.viewport;
        let again = self.counters.get().is_some();
        match self.counters.get() {
            Some(counters) => counters.write(&self.opts.cx),
            None => self
                .counters
                .set(Some(crate::layout::reuse::Counters::read(&self.opts.cx))),
        }
        let flow = FlowReuse::new(self.store.clone(), &self.nodes, allow, drawn, again);
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
