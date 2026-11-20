//! Resolve a root layout once its constraints and the previous frame are known.

use super::super::window_key::WindowKey;
use super::{Builder, UpdatePhase};
use crate::layout::reuse::FlowReuse;
use gpui::{AnyElement, App, AvailableSpace, Bounds, Pixels, Window, size};
use std::rc::Rc;

/// The actual element may be drawn under another clip or in another frame
/// than the rendering step. Its detached layout is final only in this context.
pub(in crate::document::retained) struct CheckedRoot {
    epoch: u64,
    window: WindowKey,
    stale: bool,
}

impl CheckedRoot {
    pub(super) fn new(epoch: u64, bounds: Bounds<Pixels>, window: &Window, cx: &App) -> Self {
        Self {
            epoch,
            window: WindowKey::capture(bounds, window, cx),
            stale: window.layout_stale(),
        }
    }

    pub(in crate::document::retained) fn holds(
        &self,
        epoch: u64,
        environment: Option<&WindowKey>,
        window: &Window,
    ) -> bool {
        self.epoch == epoch
            && self.stale == window.layout_stale()
            && window.retained_root_environment()
            && Some(&self.window) == environment
    }
}

impl Builder {
    pub(in crate::document::retained) fn layout_checked(
        &self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        mut observe: impl FnMut(UpdatePhase),
    ) -> (
        AnyElement,
        Option<Rc<FlowReuse>>,
        gpui::RetainedLayoutInputs,
    ) {
        let space = size(
            AvailableSpace::Definite(bounds.size.width),
            AvailableSpace::Definite(bounds.size.height),
        );
        let mut allow = bounds.origin == Default::default() && window.retained_root_environment();
        loop {
            let retry = self.counters.get().is_some();
            let (mut tree, flow) = self.build(allow, true);
            observe(UpdatePhase::Built);
            let (_, inputs) = window.record_layout_inputs(|window| {
                window.with_layout_keys(self.layout_keys && !retry, |window| {
                    tree.request_layout(window, cx);
                    observe(UpdatePhase::Requested);
                    tree.layout_as_root(space, window, cx);
                })
            });
            let refused = flow.as_ref().filter(|flow| !flow.validate(window));
            observe(UpdatePhase::LaidOut);
            if let Some(flow) = refused
                && allow
            {
                flow.note_rebuild();
                // First rebuild the refused blocks. If that layout also fails,
                // build the full tree. Other blocks keep their reuse on the first retry.
                allow = !retry;
                continue;
            }
            return (tree, flow, inputs);
        }
    }
}
