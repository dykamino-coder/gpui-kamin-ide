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

    async fn request_layout_async(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> (LayoutId, Self::Layout) {
        let lid = self.inner.request_layout_async(window, cx, stack).await;
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

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self::Prepaint {
        let id = match self.slot {
            Slot::Box(id) => Some(id),
            Slot::Table(..) => None,
        };
        // Допущенный узел записи главной копии: окна копий и израсходованную часть
        // переносят его собственные размещения и срезы.
        if let Some(slices) = id.and_then(|id| self.rec.admitted(id)) {
            admitted_prepaint(
                &mut self.inner,
                (None, gpui::Point::default()),
                slices,
                window,
                cx,
                stack,
            )
            .await;
            return None;
        }
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
        window
            .with_content_mask_async(mask, async |window| {
                window
                    .with_element_offset_async(shift, async |window| {
                        self.inner.prepaint_async(window, cx, stack).await
                    })
                    .await
            })
            .await;
        mask
    }

    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        prepaint: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        crate::hooks::paint_on_stack(self, bounds, state, prepaint, window, cx);
    }

    fn paint_async<'a>(
        &'a mut self,
        _: Bounds<Pixels>,
        _: &'a mut Self::Layout,
        prepaint: &'a mut Self::Prepaint,
        window: &'a mut Window,
        cx: &'a mut App,
        stack: &'a mut gpui::PaintStack,
    ) -> gpui::PaintFuture<'a> {
        Box::pin(self.paint_children(prepaint, window, cx, stack))
    }
}

/// Prepaint узла со своим допуском в записи главной копии (`Watched::admitted`):
/// продолжение сохраняет допуск до завершения обхода ребёнка.
async fn admitted_prepaint(
    inner: &mut AnyElement,
    (mask, shift): (Option<ContentMask<Pixels>>, gpui::Point<Pixels>),
    slices: Rc<[gpui::PaintSegmentSlice]>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) {
    window
        .with_content_mask_async(mask, async |window| {
            window
                .with_element_offset_async(shift, async |window| {
                    let _ = window
                        .with_paint_segment_slices_async(slices, async |window| {
                            inner.prepaint_async(window, cx, stack).await;
                        })
                        .await;
                })
                .await;
        })
        .await;
}

mod paint;
#[cfg(test)]
mod tests;
