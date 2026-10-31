//! Единая HTML-сетка слотов: начало ячейки, охваты и занятость рядов группы.

use super::anon::TableRow;
use super::spans::{column_span, row_span_in_group};
use crate::dom::{Element, Node};

#[derive(Clone, Copy)]
pub(super) struct CellSlot<'a> {
    pub element: &'a Element,
    pub column: usize,
    pub colspan: u16,
    pub rowspan: usize,
}

pub(super) struct GridRow<'a> {
    pub element: &'a Element,
    pub group: Option<&'a Element>,
    pub cells: Vec<CellSlot<'a>>,
    pub covered: bool,
}

pub(super) struct TableGrid<'a> {
    pub rows: Vec<GridRow<'a>>,
    pub columns: usize,
}

pub(super) fn rows_left(rows: &[TableRow<'_>]) -> Vec<usize> {
    let key = |i: usize| rows[i].carry.group.map(std::ptr::from_ref);
    let mut out = vec![1; rows.len()];
    let mut end = rows.len();
    for i in (0..rows.len()).rev() {
        if i + 1 < rows.len() && key(i + 1) != key(i) {
            end = i + 1;
        }
        out[i] = end - i;
    }
    out
}

impl<'a> TableGrid<'a> {
    pub fn new(rows: &[TableRow<'a>]) -> Self {
        let left = rows_left(rows);
        let mut occupied = Vec::<usize>::new();
        let mut grid = Self {
            rows: Vec::with_capacity(rows.len()),
            columns: 1,
        };
        for (r, record) in rows.iter().enumerate() {
            let mut row = GridRow {
                element: record.element,
                group: record.carry.group,
                cells: Vec::new(),
                covered: occupied.iter().any(|end| *end > r),
            };
            let mut column = 0usize;
            for child in &record.element.children {
                let Node::Element(cell) = child else { continue };
                if !super::is_cell(cell) {
                    continue;
                }
                // HTML пропускает занятые начала ячеек, не весь прямоугольник colspan.
                while occupied.get(column).is_some_and(|end| *end > r) {
                    column = column.checked_add(1).expect("allocated table column");
                }
                let colspan = column_span(cell);
                let rowspan = row_span_in_group(cell, left[r]);
                let end = column
                    .checked_add(usize::from(colspan))
                    .expect("allocated table columns");
                let row_end = r
                    .checked_add(rowspan)
                    .expect("rowspan bounded by group rows");
                if end > occupied.len() {
                    occupied.resize(end, 0);
                }
                for slot in &mut occupied[column..end] {
                    // Перекрытие в ошибочной разметке не сокращает прежний rowspan.
                    *slot = (*slot).max(row_end);
                }
                row.cells.push(CellSlot {
                    element: cell,
                    column,
                    colspan,
                    rowspan,
                });
                grid.columns = grid.columns.max(end);
                column = end;
            }
            grid.rows.push(row);
        }
        grid
    }
}

/// Сужение только на границе GPUI: положительная линия не становится отрицательной.
pub(super) fn grid_line(line: usize) -> i16 {
    i16::try_from(line.max(1)).unwrap_or(i16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::table::anon::{RowCarry, anon_element, collect_rows};

    fn cell(colspan: &str, rowspan: &str) -> Node {
        let mut e = anon_element("td", vec![]);
        e.attrs = vec![
            ("colspan".into(), colspan.into()),
            ("rowspan".into(), rowspan.into()),
        ];
        Node::Element(e)
    }
    fn row(cells: Vec<Node>) -> Node {
        Node::Element(anon_element("tr", cells))
    }
    fn check(nodes: &[Node], expected: &[&[(usize, u16, usize)]], columns: usize) {
        let mut rows = vec![];
        collect_rows(nodes, None, RowCarry::default(), &mut rows);
        let grid = TableGrid::new(&rows);
        assert_eq!(grid.columns, columns);
        for (actual, expected) in grid.rows.iter().zip(expected) {
            let slots: Vec<_> = actual
                .cells
                .iter()
                .map(|c| (c.column, c.colspan, c.rowspan))
                .collect();
            assert_eq!(slots, *expected);
        }
    }

    #[test]
    fn staircase_counts_slots_occupied_by_earlier_rows() {
        let nodes = [
            row(vec![cell("1", "3"), cell("1", "1")]),
            row(vec![cell("1", "2"), cell("1", "1")]),
            row(vec![cell("1", "1"), cell("1", "1")]),
        ];
        check(
            &nodes,
            &[
                &[(0, 1, 3), (1, 1, 1)],
                &[(1, 1, 2), (2, 1, 1)],
                &[(2, 1, 1), (3, 1, 1)],
            ],
            4,
        );
    }

    #[test]
    fn zero_and_large_spans_end_at_the_actual_group_even_with_anonymous_ids() {
        let nodes = [
            Node::Element(anon_element(
                "tbody",
                vec![
                    row(vec![cell("1", "0"), cell("1001", "65536")]),
                    row(vec![cell("1", "1")]),
                ],
            )),
            Node::Element(anon_element("tbody", vec![row(vec![cell("1", "0")])])),
        ];
        check(
            &nodes,
            &[&[(0, 1, 2), (1, 1000, 2)], &[(1001, 1, 1)], &[(0, 1, 1)]],
            1002,
        );
    }

    #[test]
    fn malformed_overlap_keeps_the_longer_incoming_span() {
        let nodes = [
            row(vec![cell("1", "1"), cell("1", "3")]),
            row(vec![cell("2", "1")]),
            row(vec![cell("1", "1"), cell("1", "1")]),
        ];
        check(
            &nodes,
            &[
                &[(0, 1, 1), (1, 1, 3)],
                &[(0, 2, 1)],
                &[(0, 1, 1), (2, 1, 1)],
            ],
            3,
        );
    }

    #[test]
    fn large_grid_lines_do_not_wrap() {
        assert_eq!(grid_line(32767), 32767);
        assert_eq!(grid_line(65536), 32767);
        assert_eq!(grid_line(usize::MAX), 32767);
    }
}
