//! Элемент BandFlow: раскладка детей по полосам при request_layout/prepaint.

mod measure;
mod paint;

use super::kid::Kid;
use crate::hooks::Hooks;
use crate::layout::build_contracts::ChildBuild;
use crate::layout::cx::LayoutCx;
use crate::layout::cx::deferred::Deferred;
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window};
use std::rc::Rc;

pub struct BandFlow {
    pub(super) paint: Rc<Deferred>,
    pub(super) build: ChildBuild,
    pub(super) kids: Rc<Vec<Kid>>,
    pub(super) plan: Rc<super::cache::Plans>,
    intrinsic: Rc<std::cell::Cell<Option<(f32, f32)>>>,
    pub(super) fill_inline: bool,
    pub(super) built: Vec<AnyElement>,
    /// Письмо хоста: `Some(true)` — `vertical-rl`, `Some(false)` — `vertical-lr`.
    pub(super) vert: Option<bool>,
    pub(super) inline_up: bool,
    pub(super) contain_floats: bool,
    pub(super) clamp: Option<(u64, crate::text::clamp::ClampMeasure)>,
    pub(super) fragment: Option<super::fragment::FragmentFlow>,
    pub(super) positioning: Option<Rc<crate::layout::positioned::containing_block::Positioning>>,
}

impl BandFlow {
    pub(crate) fn new(kids: Vec<Kid>, contain_floats: bool, cx: &LayoutCx) -> Self {
        BandFlow {
            paint: cx.deferred.paint.clone(),
            build: cx.contracts.build,
            kids: Rc::new(kids),
            plan: Default::default(),
            intrinsic: Default::default(),
            fill_inline: cx.constraints.avail_w.is_some(),
            built: Vec::new(),
            vert: None,
            inline_up: false,
            contain_floats,
            fragment: None,
            clamp: None,
            positioning: None,
        }
    }

    /// Host block direction and line-left edge (Writing Modes 4 §6.4).
    pub fn vertical(mut self, wm: crate::style::computed::writing_mode::WritingMode) -> Self {
        self.vert = Some(wm.block_rl());
        self.inline_up = wm.sideways_lr();
        self
    }
}

impl Hooks for BandFlow {
    type Layout = LayoutId;
    type Prepaint = ();

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        _cx: &mut App,
        _stack: &mut gpui::PaintStack,
    ) -> (LayoutId, Self::Layout) {
        let id = self.request_plan(window);
        (id, id)
    }

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        self.prepaint_plan(bounds, state, window, cx, stack).await;
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        _prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, _bounds, _state, _prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _bounds: Bounds<Pixels>,
        _state: &'a mut Self::Layout,
        _prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(self.paint_children(window, cx, stack))
    }
}

#[cfg(test)]
#[path = "element_tests.rs"]
mod tests;
