//! Геометрия наблюдаемой копии: нормальные flex-позиции и независимые потоки.
use super::Watched;
use crate::layout::fragment::flow::blocks::Laid;
use gpui::{Pixels, Point, Window};
use std::collections::HashMap;

impl Watched {
    /// Laid-out geometry of the copy relative to `origin`, read before the
    /// layout engine is released: border boxes `(top, bottom)` of the watched
    /// boxes and frames of the watched tables.
    pub(crate) fn laid(&self, origin: Point<Pixels>, window: &mut Window) -> Laid {
        let mut boxes = self
            .seen
            .borrow()
            .iter()
            .map(|(&id, &lid)| {
                let top = f32::from(window.layout_origin_unrounded(lid).y - origin.y);
                let h = f32::from(window.layout_size_unrounded(lid).height);
                (id, (top, top + h))
            })
            .collect();
        let super::flex::FlexFrames {
            flexes,
            shifts,
            table_shifts,
        } = self.flex_frames(origin, window, &mut boxes);
        let mut tables = self.table_frames(origin, window);
        let mut grids = self.rows(origin, window);
        let mut flows = self.item_geometry(origin, window);
        let mut lines: HashMap<_, _> = self
            .item_flows
            .iter()
            .map(|(&id, flow)| (id, flow.line_pushes(origin, window)))
            .collect();
        for (id, delta) in shifts {
            if let Some(lines) = lines.get_mut(&id) {
                for p in lines {
                    p.at += delta;
                }
            }
            if let Some(rows) = grids.get_mut(&id) {
                for (a, b) in rows {
                    *a += delta;
                    *b += delta;
                }
            }
            if let Some(flow) = flows.get_mut(&id) {
                let mut shifted = crate::layout::fragment::flow::FlowGeometry::default();
                shifted.append(flow, delta);
                *flow = shifted;
            }
        }
        for (id, delta) in table_shifts {
            if let Some(t) = tables.get_mut(&id) {
                t.table.0 += delta;
                t.table.1 += delta;
                t.grid.0 += delta;
                t.grid.1 += delta;
                for (a, b) in &mut t.rows {
                    *a += delta;
                    *b += delta;
                }
            }
        }
        for (&id, &offset) in self.flex_offsets.borrow().iter() {
            if let Some(&(top, _)) = boxes.get(&id) {
                boxes.insert(
                    id ^ crate::layout::fragment::flow::blocks::STRUT_ID,
                    (top - offset, top),
                );
            }
        }
        Laid {
            boxes,
            tables,
            grids,
            flexes,
            flows,
            lines,
        }
    }
}
