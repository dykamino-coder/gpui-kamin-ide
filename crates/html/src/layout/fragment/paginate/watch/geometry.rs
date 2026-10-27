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
            .filter_map(|(&id, &node)| {
                let bounds = node.bounds(window)?;
                Some((id, projection.span(bounds.origin, bounds.size)))
            })
            .collect();
        let super::flex_geometry::FlexFrames { flexes, margins } =
            self.flex_frames(projection, window, &mut boxes);
        let tables = self.table_frames(projection, window);
        let super::grid::GridFrames {
            rows: grids,
            placements,
        } = self.grid_frames(projection, window);
        let flows = self.item_geometry(projection, window);
        let content: HashMap<_, _> = self
            .item_flows
            .iter()
            .map(|(&id, flow)| (id, flow.content_lines_axis(origin, size, axis, window)))
            .collect();
        let mut lines: HashMap<_, _> = self
            .item_flows
            .iter()
            .map(|(&id, flow)| (id, flow.line_pushes_axis(origin, size, axis, window)))
            .collect();
        if let Some(blocks) = &self.blocks {
            // Строки обычного потока тоже должны принимать распорки роста.
            // Иначе finish видит их разрыв, но pushed_at не находит его строку.
            lines
                .entry(blocks.id)
                .or_default()
                .extend(self.flow.line_pushes_axis(origin, size, axis, window));
        }
        // Распорка роста стоит перед margin box элемента: поле после
        // принудительного разрыва остаётся при нём (css-break-3 §5.2).
        // Отрицательный сдвиг (погашенный промежуток) распорки не даёт.
        for (&id, &offset) in self.flex_offsets.borrow().iter().filter(|o| *o.1 > 0.0) {
            if let Some(&(top, _)) = boxes.get(&id) {
                let start = top - margins.get(&id).map_or(0.0, |m| m.0);
                boxes.insert(
                    id ^ crate::layout::fragment::flow::blocks::STRUT_ID,
                    (start - offset, start),
                );
            }
        }
        let mut descendants = HashMap::new();
        if let Some(blocks) = &self.blocks {
            blocks.descendants(&mut descendants);
        }
        let mut laid = Laid {
            boxes,
            tables,
            grids,
            placements,
            row_growth: self.grid_growth.borrow().clone(),
            flexes,
            flows,
            lines,
            content,
            descendants,
            margins,
        };
        if let Some(blocks) = &self.blocks {
            blocks.subgrid_growth(&mut laid);
        }
        laid
    }
}
