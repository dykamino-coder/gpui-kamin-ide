//! Resolve physical CSS scroll ranges, including far-edge origins.
use super::geometry::{sides, union};
use super::*;
use crate::dom::Node;
use gpui::ScrollHandle;
impl Frame {
    pub(crate) fn settle(
        &self,
        tree: &Element,
        c: &Computed,
        handle: &ScrollHandle,
        bounds: Bounds<Pixels>,
    ) -> bool {
        if !self.marker.get() {
            return false;
        }
        let boxes = self.boxes.borrow();
        let text_boxes = self.text_boxes.borrow();
        fn area(
            e: &Element,
            boxes: &HashMap<u64, (Rect, Clip)>,
            text: &HashMap<u64, Rect>,
        ) -> Option<Rect> {
            let descendants = e
                .children
                .iter()
                .filter_map(|node| match node {
                    Node::Element(child) => area(child, boxes, text),
                    _ => None,
                })
                .chain(text.get(&e.node_id).copied())
                .reduce(union);
            let Some(&(own, clip)) = boxes.get(&e.node_id) else {
                return descendants;
            };
            Some(
                descendants
                    .and_then(|r| clip.intersect(own, r))
                    .map_or(own, |r| union(own, r)),
            )
        }
        let border = sides(c.borders(), 0.0);
        let padding = sides(c.padding, f32::from(bounds.size.width));
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
        let offset = handle.offset();
        let mut end = [origin[0] + view[0], origin[1] + view[1]];
        let mut start = origin;
        let reversed = crate::interactive::scrollbar_gutter::reversed(c);
        for node in &tree.children {
            if let Node::Element(e) = node
                && let Some(rect) = area(e, &boxes, &text_boxes)
            {
                let margin = sides(e.style.margin, view[0]);
                for axis in 0..2 {
                    start[axis] = start[axis].min(
                        rect[axis]
                            - f32::from(if axis == 0 { offset.x } else { offset.y })
                            - (padding[axis] + margin[axis].min(0.0)).max(0.0),
                    );
                }
                // A negative end margin can consume end padding, but never
                // remove the descendant's visible border box from overflow.
                end[0] = end[0].max(
                    rect[2] - f32::from(offset.x) + (padding[2] + margin[2].min(0.0)).max(0.0),
                );
                end[1] = end[1].max(
                    rect[3] - f32::from(offset.y) + (padding[3] + margin[3].min(0.0)).max(0.0),
                );
            }
        }
        if let Some(rect) = text_boxes.get(&tree.node_id) {
            for axis in 0..2 {
                let shift = f32::from(if axis == 0 { offset.x } else { offset.y });
                start[axis] = start[axis].min(rect[axis] - shift - padding[axis]);
                end[axis] = end[axis].max(rect[axis + 2] - shift + padding[axis + 2]);
            }
        }
        // The proxy is child zero. Native children also cover text-only runs
        // and CSS boxes whose own nested scrollport owns their descendants.
        if tree
            .children
            .iter()
            .any(|n| matches!(n, Node::Text(t) if !t.trim().is_empty()))
            && let Some(marker) = handle.bounds_for_item(0)
        {
            for index in 1..handle.children_count() {
                if let Some(child) = handle.bounds_for_item(index)
                    && !classic::decoration(child, bounds, gutter)
                {
                    end[0] = end[0]
                        .max(origin[0] + f32::from(child.right() - marker.left()) + padding[2]);
                    end[1] = end[1]
                        .max(origin[1] + f32::from(child.bottom() - marker.top()) + padding[3]);
                }
            }
        }
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
        if changed || bars_changed {
            return true;
        }
        if c.snap.axis != 0 && !self.snapped.get() && !self.snap_areas.borrow().is_empty() {
            self.snapped.set(true);
            let mut areas: Vec<_> = self
                .snap_areas
                .borrow()
                .iter()
                .map(|(&id, &area)| (id, area))
                .collect();
            areas.sort_by_key(|(id, _)| *id);
            return snap::initial(c, areas.into_iter().map(|(_, area)| area), handle);
        }
        false
    }
}
