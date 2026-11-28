//! Сведение измеренных за кадр коробок в физический диапазон прокрутки CSS (Overflow 3 §2.2):
//! объединение потомков через обрезки предков, сеточные дорожки (CSS Grid §5.3), текст,
//! отрицательные поля и начало у дальнего края; результат переводится в размер содержимого
//! GPUI, обновляет полосы и запускает начальный scroll-snap.

use super::geometry::sides;
use super::*;
use crate::dom::Node;
use crate::style::computed::Position;
use crate::values::ValueCx;
use gpui::ScrollHandle;

mod native;

impl Frame {
    pub(crate) fn settle(
        &self,
        tree: &Element,
        c: &Computed,
        handle: &ScrollHandle,
        bounds: Bounds<Pixels>,
        sticky: Option<&super::exact_probe::StickyShift>,
        vcx: ValueCx<'_>,
    ) -> bool {
        if self.marker.borrow().is_empty() {
            // No proxy: the range the prepaint recorded is its content's.
            return handle.clamp_to_range();
        }
        // Probes exclude ancestor sticky/scroll projections. Use the same
        // coordinate space for a nested scrollport's origin (Overflow 3 §2.2).
        let snap_bounds = bounds;
        let mut bounds = bounds;
        bounds.origin -= super::exact_probe::sticky_shift(sticky);
        let records = self.boxes.borrow();
        let boxes: HashMap<_, _> = records.iter().map(|r| (r.id, (r.rect, r.clip))).collect();
        let snap_areas: HashMap<_, _> = records
            .iter()
            .filter_map(|r| r.snap.map(|a| (r.id, a)))
            .collect();
        let text_boxes = self.text_boxes.borrow();
        use self::native::area;
        let border = sides(c.borders(), 0.0);
        let padding = sides(c.reset_box_layout.padding, f32::from(bounds.size.width));
        let mut origin = [
            f32::from(bounds.left()) + border[0],
            f32::from(bounds.top()) + border[1],
        ];
        let gutter = self.reserved.get();
        let native = self.native_reserved.get();
        let insets = self.insets.get();
        origin[0] += insets[0];
        origin[1] += insets[1];
        let view = [
            f32::from(bounds.size.width) - border[0] - border[2] - gutter[0],
            f32::from(bounds.size.height) - border[1] - border[3] - gutter[1],
        ];
        let offset = handle.painted_offset(self.scale.get());
        let mut end = [origin[0] + view[0], origin[1] + view[1]];
        // DevTools `why`: what reaches the far end of the range on each axis.
        let mut far = [(("the view", tree.node_id), 0.0f32); 2];
        let mut reach = |end: &mut [f32; 2], axis: usize, at: f32, what: (&'static str, u64)| {
            if at > end[axis] {
                end[axis] = at;
                far[axis] = (what, at - origin[axis]);
            }
        };
        let mut start = origin;
        let reversed = crate::interactive::scrollbar_gutter::reversed(c);
        // CSS Grid §5.3 includes the grid itself, even with no grid items.
        // Blink GridLayoutAlgorithm sets inflow bounds to the track span.
        if let Some(rect) = self.grid_bounds.borrow().iter().rev().find_map(|r| *r) {
            for axis in 0..2 {
                let shift = f32::from(if axis == 0 { offset.x } else { offset.y });
                start[axis] = start[axis].min(rect[axis] - shift - padding[axis]);
                reach(
                    &mut end,
                    axis,
                    rect[axis + 2] - shift + padding[axis + 2],
                    ("the grid", tree.node_id),
                );
            }
        }
        // Positioned children as the native children loop below sees them.
        let mut positioned = Vec::new();
        for node in &tree.children {
            if let Node::Element(e) = node
                && let Some(rect) = area(e, &boxes, &text_boxes)
            {
                let used = e.style.used();
                let margin = sides(used.reset_box_layout.margin, view[0]);
                // The end padding follows the in-flow content only (CSS Overflow 3
                // §2.2, Blink `inflow_bounds`): an absolutely positioned child
                // brings its border box alone, as Taffy's scrollable overflow does.
                let inflow = !matches!(
                    used.reset_positioned.position,
                    Some(Position::Absolute | Position::Fixed)
                );
                let pad = |side: usize| {
                    if inflow {
                        (padding[side] + margin[side].min(0.0)).max(0.0)
                    } else {
                        0.0
                    }
                };
                for axis in 0..2 {
                    start[axis] = start[axis].min(
                        rect[axis]
                            - f32::from(if axis == 0 { offset.x } else { offset.y })
                            - pad(axis),
                    );
                }
                // A negative end margin can consume end padding, but never
                // remove the descendant's visible border box from overflow.
                if !inflow && let Some(&(own, _)) = boxes.get(&e.node_id) {
                    let (x, y) = (f32::from(offset.x), f32::from(offset.y));
                    positioned.push([own[0] - x, own[1] - y, own[2] - x, own[3] - y]);
                }
                let what = if inflow {
                    "an in-flow child box"
                } else {
                    "a positioned child box"
                };
                reach(
                    &mut end,
                    0,
                    rect[2] - f32::from(offset.x) + pad(2),
                    (what, e.node_id),
                );
                reach(
                    &mut end,
                    1,
                    rect[3] - f32::from(offset.y) + pad(3),
                    (what, e.node_id),
                );
            }
        }
        if let Some(rect) = text_boxes.get(&tree.node_id) {
            for axis in 0..2 {
                let shift = f32::from(if axis == 0 { offset.x } else { offset.y });
                start[axis] = start[axis].min(rect[axis] - shift - padding[axis]);
                reach(
                    &mut end,
                    axis,
                    rect[axis + 2] - shift + padding[axis + 2],
                    ("its own lines", tree.node_id),
                );
            }
        }
        self::native::reach_native(
            tree,
            handle,
            bounds,
            gutter,
            origin,
            padding,
            &positioned,
            &mut end,
            &mut reach,
        );
        self.range.set(std::array::from_fn(|axis| {
            let range = if reversed[axis] {
                origin[axis] - start[axis]
            } else {
                end[axis] - origin[axis] - view[axis]
            }
            .max(0.0);
            if range <= 0.5 { 0.0 } else { range }
        }));
        let bars_changed = classic::update(self, c, view);
        // GPUI adds padding and subtracts the outer border box. Convert the
        // CSS padding-box overflow extent into that content-size convention.
        let extent = size(
            px(
                (view[0] + self.range.get()[0] + border[0] + border[2] - padding[0] - padding[2]
                    + native[0])
                    .max(0.0),
            ),
            px(
                (view[1] + self.range.get()[1] + border[1] + border[3] - padding[1] - padding[3]
                    + native[1])
                    .max(0.0),
            ),
        );
        let changed = self.extent.replace(extent) != extent;
        // A new extent with the same bars changes no box this layout placed:
        // the scroll range takes the proxy's new size in place (as Blink's
        // `PaintLayerScrollableArea::UpdateAfterLayout` updates the range
        // after layout without laying out again). New bars change the
        // content box, and an offset past the measured range is clamped to
        // it (the prepaint kept the offset asked for,
        // `ScrollHandle::measure_range_after_prepaint`; Blink's
        // `ClampScrollOffsetAfterOverflowChange`), which moves the content:
        // those lay the scrollport out again.
        let fits = match changed {
            true => handle.resize_item(0, extent),
            false => !handle.clamp_to_range(),
        };
        if bars_changed || !fits {
            // DevTools `why`: the reason this scrollport is laid out again.
            crate::why::note("scroll_extent.relayout", || {
                let why = match (bars_changed, classic::refusal(self)) {
                    (true, Some(refusal)) => {
                        format!("bars changed, not decided in layout: {refusal}")
                    }
                    (true, None) => {
                        "bars changed: the range disagrees with the bars layout decided".into()
                    }
                    (false, _) => "the offset lies past the new range".to_string(),
                };
                format!(
                    "{why}: box {} extent {extent:?} offset {:?} range {:?} view {view:?} bars {:?} far {far:?}",
                    tree.node_id,
                    handle.offset(),
                    self.range.get(),
                    self.bars.get()
                )
            });
            return true;
        }
        if c.reset_box_layout.snap.axis != 0 && !self.snapped.get() && !snap_areas.is_empty() {
            self.snapped.set(true);
            let mut areas: Vec<_> = snap_areas.iter().map(|(&id, &area)| (id, area)).collect();
            areas.sort_by_key(|(id, _)| *id);
            return snap::initial(
                c,
                areas.into_iter().map(|(_, area)| area),
                handle,
                snap_bounds,
                self.insets.get(),
                self.range.get(),
                vcx,
            );
        }
        false
    }
}
