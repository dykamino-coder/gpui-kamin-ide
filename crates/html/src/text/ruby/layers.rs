//! Shared annotation geometry for nested ruby and emphasis outside ruby.

use gpui::{LayoutId, Window};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub(crate) type LayerSlot = Rc<Cell<Option<LayoutId>>>;
pub(crate) type LayerAttachment = (Rc<Layers>, usize);

#[derive(Debug, Default)]
pub(crate) struct Layers {
    pub(crate) nested: Cell<(f32, f32)>,
    nested_raw: Cell<(f32, f32)>,
    marks: Cell<(f32, f32)>,
    levels: RefCell<Vec<(bool, LayerSlot)>>,
}

impl Layers {
    pub(crate) fn mark(&self, under: bool, height: f32) {
        let (over, below) = self.marks.get();
        self.marks.set(if under {
            (over, below.max(height))
        } else {
            (over.max(height), below)
        });
    }

    pub(crate) fn mark_height(&self, under: bool) -> f32 {
        let h = self.marks.get();
        if under { h.1 } else { h.0 }
    }

    pub(crate) fn extra(&self, under: bool, before: usize, window: &mut Window) -> f32 {
        self.before(under, before, window) + self.mark_height(under)
    }

    pub(crate) fn add(&self, under: bool, slot: LayerSlot) -> usize {
        let mut levels = self.levels.borrow_mut();
        let before = levels.len();
        levels.push((under, slot));
        before
    }

    pub(crate) fn offset(&self, under: bool, window: &mut Window) -> f32 {
        self.before(under, self.levels.borrow().len(), window)
    }

    pub(crate) fn last_index(&self) -> usize {
        self.levels.borrow().len().saturating_sub(1)
    }

    pub(crate) fn set_nested(&self, heights: (f32, f32)) {
        self.nested_raw.set(heights);
        self.nested.set(heights);
    }

    pub(crate) fn account_leading(&self, pads: &[(f32, f32)]) {
        // The technical base paragraph already includes this leading in its
        // margin box. Only the unconsumed part moves the outer annotation.
        let raw = self.nested_raw.get();
        self.nested.set((
            (raw.0 - pads.first().map_or(0.0, |p| p.0)).max(0.0),
            (raw.1 - pads.last().map_or(0.0, |p| p.1)).max(0.0),
        ));
    }

    pub(crate) fn before(&self, under: bool, before: usize, window: &mut Window) -> f32 {
        let h = self.nested.get();
        let nested = if under { h.1 } else { h.0 };
        nested
            + self
                .levels
                .borrow()
                .iter()
                .take(before)
                .filter(|(side, _)| *side == under)
                .filter_map(|(_, id)| id.get())
                .map(|id| f32::from(window.layout_exact(id).1.height))
                .sum::<f32>()
    }
}

/// Options for a ruby base column: nested ruby annotations inside stack on
/// `layers` (`LayoutCx::ruby_layers`).
pub(crate) fn based(
    opts: &crate::layout::options::RenderOpts,
    layers: Rc<Layers>,
) -> Box<crate::layout::options::RenderOpts> {
    opts.inner(|cx| cx.ruby_layers = Some(layers))
}

/// Options for an independent inline context inside a ruby (an annotation,
/// an atomic inline): no base layers and no base-width sink.
pub(crate) fn independent(
    opts: &crate::layout::options::RenderOpts,
) -> Box<crate::layout::options::RenderOpts> {
    opts.inner(|cx| {
        cx.ruby_layers = None;
        cx.ruby_base = None;
    })
}
