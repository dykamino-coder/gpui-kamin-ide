//! Обёртка `Watch` наблюдаемой коробки: записывает внутренний узел раскладки в рекордер
//! сборки (`Watched::seen`) и отдаёт его как свой.

use super::{TablePart, Watched};
use crate::hooks::{Hooks, IntoHooked};
use crate::layout::fragment::types::StackAxis;
use crate::layout::options::RenderOpts;
use gpui::{AnyElement, App, Bounds, ContentMask, LayoutId, Pixels, Window};
use std::rc::Rc;

/// Wrap `el` when the build's recorder watches `id` (identity otherwise).
pub(crate) fn wrap(id: u64, el: AnyElement, opts: &RenderOpts) -> AnyElement {
    match &opts.frag.watch {
        Some(rec) if rec.watches(id) => Watch {
            slot: Slot::Box(id),
            inner: el,
            rec: rec.clone(),
        }
        .hooked(),
        _ => el,
    }
}

/// What a watched node is recorded as.
#[derive(Clone, Copy)]
pub(super) enum Slot {
    /// Border box of the box with this node id.
    Box(u64),
    /// Part of the table with this key (`table_part`).
    Table(u64, TablePart),
}

pub(super) struct Watch {
    pub(super) slot: Slot,
    pub(super) inner: AnyElement,
    pub(super) rec: Rc<Watched>,
}

impl Hooks for Watch {
    type Layout = ();
    type Prepaint = Option<ContentMask<Pixels>>;

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let lid = self.inner.request_layout(window, cx);
        match self.slot {
            Slot::Box(id) => {
                window.set_fragmentation_monolithic(lid, self.rec.monolithic(id));
                if self.rec.flex_container(id) {
                    window.set_flex_fragmentation(lid, true);
                }
                let axis = self.rec.axis();
                let block = super::axis::solver_axis(axis);
                window.set_flex_fragmentation_axis(lid, block, axis == StackAxis::VerticalRl);
                if let Some(growth) = self.rec.flex_growth.borrow().get(&id) {
                    window.set_flex_line_fragmentation_growth(lid, growth);
                }
                if let Some(growth) = self.rec.grid_growth.borrow().get(&id) {
                    window.set_grid_fragmentation_growth(lid, block, growth);
                }
                if let Some(&line) = self.rec.flex_lines.borrow().get(&id) {
                    window.set_flex_item_fragmentation_line(lid, Some(line));
                }
                if let Some(offset) = self.rec.flex_offsets.borrow().get(&id) {
                    window.set_flex_item_fragmentation_offset(lid, *offset);
                }
                if let Some(consumed) = self.rec.flex_consumed.borrow().get(&id) {
                    window.set_flex_item_fragmentation_consumed(lid, consumed.0);
                }
                self.rec.seen.borrow_mut().insert(
                    id,
                    crate::layout::fragment::geometry::Node::capture(lid, window),
                );
            }
            Slot::Table(key, part) => self.rec.record_part(key, part, lid, window),
        }
        (lid, ())
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::Prepaint {
        let id = match self.slot {
            Slot::Box(id) => Some(id),
            Slot::Table(..) => None,
        };
        let consumed = id.and_then(|id| self.rec.flex_consumed.borrow().get(&id).copied());
        let item = id.and_then(|id| self.rec.item_window(id));
        let mask = consumed
            .map(|(consumed, complete)| {
                super::axis::consumed_mask(
                    self.rec.axis(),
                    window.content_mask(),
                    bounds,
                    consumed,
                    complete,
                )
            })
            .or_else(|| {
                let clip = item?.clip?;
                let current = window.content_mask().bounds;
                let bottom = (bounds.top() + gpui::px(clip)).min(current.bottom());
                Some(ContentMask {
                    bounds: Bounds::from_corners(
                        current.origin,
                        gpui::point(current.right(), bottom.max(current.top())),
                    ),
                })
            });
        let shift = gpui::point(gpui::px(0.0), gpui::px(item.map_or(0.0, |w| w.shift)));
        if let Some(slices) = id.and_then(|id| self.rec.admitted(id)) {
            admitted_prepaint(&mut self.inner, (mask, shift), slices, window, cx);
            return mask;
        }
        window.with_content_mask(mask, |window| {
            window.with_element_offset(shift, |window| self.inner.prepaint(window, cx))
        });
        mask
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let access = match self.slot {
            Slot::Box(id) => self.rec.access(id, window.content_mask()),
            Slot::Table(..) => super::owner::Access::Normal,
        };
        if let Slot::Box(id) = self.slot
            && let Some(slices) = self.rec.admitted(id)
        {
            admitted_paint(&mut self.inner, *prepaint, slices, window, cx);
            return;
        }
        match access {
            super::owner::Access::Hidden => {}
            super::owner::Access::Escape(mask) => window
                .with_content_mask_replaced(mask, |window| {
                    window.with_content_mask(*prepaint, |window| self.inner.paint(window, cx))
                }),
            super::owner::Access::Normal => {
                window.with_content_mask(*prepaint, |window| self.inner.paint(window, cx))
            }
        }
    }
}

/// Prepaint узла со своим допуском в записи главной копии (`Watched::admitted`):
/// отдельный кадр, вне пути каждого наблюдаемого узла.
#[inline(never)]
fn admitted_prepaint(
    inner: &mut AnyElement,
    (mask, shift): (Option<ContentMask<Pixels>>, gpui::Point<Pixels>),
    slices: Rc<[gpui::PaintSegmentSlice]>,
    window: &mut Window,
    cx: &mut App,
) {
    window.with_content_mask(mask, |window| {
        window.with_element_offset(shift, |window| {
            let _ = window.with_paint_segment_slices(slices, |window| inner.prepaint(window, cx));
        })
    });
}

/// Краска узла со своим допуском в записи главной копии.
#[inline(never)]
fn admitted_paint(
    inner: &mut AnyElement,
    mask: Option<ContentMask<Pixels>>,
    slices: Rc<[gpui::PaintSegmentSlice]>,
    window: &mut Window,
    cx: &mut App,
) {
    let _ = window.with_paint_segment_slices(slices, |window| {
        window.with_content_mask(mask, |window| inner.paint(window, cx))
    });
}
