//! Геометрия наблюдаемой копии: нормальные flex-позиции и независимые потоки.
use super::{Watched, axis::BlockAxis};
use crate::layout::fragment::flow::blocks::Laid;
use gpui::{Pixels, Point, Window};
use std::collections::HashMap;

impl Watched {
    /// Физические border box в логической блочной оси вертикального потока.
    pub(crate) fn laid_axis(
        &self,
        origin: Point<Pixels>,
        size: gpui::Size<Pixels>,
        axis: crate::layout::fragment::types::StackAxis,
        window: &mut Window,
    ) -> Laid {
        let projection = BlockAxis { origin, size, axis };
        let mut boxes = self
            .seen
            .borrow()
            .iter()
            .map(|(&id, &lid)| {
                (
                    id,
                    projection.span(
                        window.layout_origin_unrounded(lid),
                        window.layout_size_unrounded(lid),
                    ),
                )
            })
            .collect();
        let super::flex::FlexFrames {
            flexes,
            shifts,
            table_shifts,
        } = self.flex_frames(projection, window, &mut boxes);
        let mut tables = self.table_frames(projection, window);
        let mut grids = self.rows(projection, window);
        let mut flows = self.item_geometry(projection, window);
        let mut content: HashMap<_, _> = self
            .item_flows
            .iter()
            .map(|(&id, flow)| (id, flow.content_lines_axis(origin, size, axis, window)))
            .collect();
        let mut lines: HashMap<_, _> = self
            .item_flows
            .iter()
            .map(|(&id, flow)| (id, flow.line_pushes_axis(origin, size, axis, window)))
            .collect();
        for (id, delta) in shifts {
            if let Some(lines) = content.get_mut(&id) {
                for l in lines {
                    l.top += delta;
                    l.bottom += delta;
                }
            }
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
        let mut descendants = HashMap::new();
        if let Some(blocks) = &self.blocks {
            blocks.descendants(&mut descendants);
        }
        Laid {
            boxes,
            tables,
            grids,
            flexes,
            flows,
            lines,
            content,
            descendants,
        }
    }
}
