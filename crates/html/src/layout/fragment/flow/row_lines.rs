//! Строки физического ряда (абзац из слов и атомарных строчных коробок, разложенный
//! переносящимся рядом): линии ряда — его line boxes (CSS 2.1 §9.4.2). Разрыв возможен между
//! ними (css-break-3 §4.1 class B), внутри линии — нет: атомарная коробка монолитна (§4.1).
//! Линии известны только после раскладки, поэтому рекордер хранит узлы кусков ряда, а
//! геометрию собирает при чтении.

use super::{FlowGeometry, FlowMeasure, Record};
use crate::layout::fragment::geometry::Node;
use crate::layout::fragment::types::StackAxis;
use crate::tolerance::COORD_EPS;
use gpui::{Bounds, LayoutId, Pixels, Window};

/// Узлы кусков ряда и `orphans`/`widows` его блока.
#[derive(Debug)]
pub(crate) struct RowLines {
    pieces: Vec<Node>,
    keep: (usize, usize),
}

impl FlowMeasure {
    /// Ряд `id` из кусков `pieces`: его линии читаются вместе с прочими строками.
    pub(crate) fn record_row(
        &self,
        id: LayoutId,
        pieces: &[LayoutId],
        axis: StackAxis,
        keep: (usize, usize),
        window: &Window,
    ) {
        let row = RowLines {
            pieces: pieces.iter().map(|&p| Node::capture(p, window)).collect(),
            keep,
        };
        self.records.record(id, Record::row(axis, row), window);
    }
}

impl RowLines {
    /// Линии ряда с рамкой `row` в блочной оси самого ряда.
    pub(super) fn geometry(
        &self,
        row: Bounds<Pixels>,
        axis: StackAxis,
        window: &Window,
    ) -> FlowGeometry {
        let ranges = self.pieces.iter().filter_map(|piece| {
            let b = piece.bounds(window)?;
            let start =
                super::coordinates::block_offset(row.origin, row.size, b.origin, b.size, axis);
            let extent = if axis.is_vertical() {
                b.size.width
            } else {
                b.size.height
            };
            Some((start, start + f32::from(extent)))
        });
        lines(ranges.collect(), self.keep)
    }
}

/// Линии из блочных охватов кусков: перекрывающиеся охваты — одна линия.
fn lines(mut ranges: Vec<(f32, f32)>, (orphans, widows): (usize, usize)) -> FlowGeometry {
    ranges.retain(|(a, b)| a.is_finite() && b.is_finite() && b > a);
    ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (a, b) in ranges {
        match merged.last_mut() {
            Some(line) if a < line.1 - COORD_EPS => line.1 = line.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    let mut geometry = FlowGeometry::default();
    let mut end = 0.0;
    for &(a, b) in &merged {
        geometry.solid.push((a, b));
        // A gap before the line (clearance, struts) also ends the previous one.
        if a > end + COORD_EPS {
            geometry.cuts.push((a, a));
        }
        end = b;
        geometry.cuts.push((b, b));
    }
    geometry.keep_lines(orphans, widows);
    geometry
}

#[cfg(test)]
mod tests {
    use super::lines;

    #[test]
    fn pieces_on_one_line_form_one_unbreakable_line() {
        // Atom 0..60 with a word 40..58 on the same line.
        let g = lines(vec![(0., 60.), (40., 58.)], (1, 1));
        assert_eq!(g.solid, [(0., 60.)]);
        assert_eq!(g.cuts, [(60., 60.)]);
    }

    #[test]
    fn line_boxes_of_a_wrapped_row_are_break_opportunities() {
        let g = lines(vec![(0., 20.), (20., 40.), (40., 60.)], (1, 1));
        assert_eq!(g.solid, [(0., 20.), (20., 40.), (40., 60.)]);
        assert_eq!(g.cuts, [(20., 20.), (40., 40.), (60., 60.)]);
        // orphans/widows 2 keep the first two and the last two together.
        let g = lines(vec![(0., 20.), (20., 40.), (40., 60.)], (2, 2));
        assert_eq!(g.solid, [(0., 60.)]);
    }
}
