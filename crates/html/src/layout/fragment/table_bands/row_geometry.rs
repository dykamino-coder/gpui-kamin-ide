//! Shared row/cell geometry for table measurement and continuation probes.
//! Following CSS Tables 3 fragmentation, a spanning cell can expand its row
//! when row and span sizes match. Blink applies this encompassing check in
//! table_row_layout_algorithm.cc:112-118; spans over other non-empty
//! rows keep their independently sized row tracks.

use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::table::is_cell;
use crate::style::values::value::Len;
use crate::tolerance::COORD_EPS;

pub(crate) struct CellGeometry<'a> {
    pub(crate) cell: &'a Element,
    pub(crate) shape: Shape,
    pub(crate) span: usize,
    pub(crate) expandable: bool,
}

pub(crate) struct RowGeometry<'a> {
    pub(crate) height: f32,
    pub(crate) cells: Vec<CellGeometry<'a>>,
}

/// Resolve spans against their row group before using their height or breaks.
/// A span crossing populated rows is still opaque to the continuation probe:
/// its contents cannot grow the originating row (Blink's encompassing test).
pub(crate) fn row_geometry<'a>(
    rows: &[(usize, &'a Element)],
    depth: u8,
    cx: ShapeCx,
    spacing: f32,
    vcx: ValueCx<'_>,
) -> Option<Vec<RowGeometry<'a>>> {
    let mut out = Vec::with_capacity(rows.len());
    for (i, &(group, row)) in rows.iter().enumerate() {
        let mut height = match row.style.height {
            None => 0.0,
            Some(Len::Px(h)) => h,
            _ => return None,
        };
        let left = rows[i..].iter().take_while(|r| r.0 == group).count();
        let mut cells = Vec::new();
        for n in row.children.iter().filter(|n| !is_blank(n)) {
            let Node::Element(cell) = n else { return None };
            if !is_cell(cell) {
                return None;
            }
            // HTML table model, matching table::row_span_in_group: invalid
            // attributes mean one row; zero means the rest of this group.
            let span = match cell
                .attr("rowspan")
                .and_then(|v| v.trim().parse::<usize>().ok())
            {
                Some(0) => left,
                Some(n) => n.clamp(1, left),
                None => 1,
            };
            let shape = shape_full(cell, depth.checked_sub(1)?, cx, vcx)?;
            if span == 1 {
                height = height.max(shape.height);
            }
            cells.push(CellGeometry {
                cell,
                shape,
                span,
                expandable: span == 1,
            });
        }
        out.push(RowGeometry { height, cells });
    }
    // Only collapse empty trailing rows with zero spacing. Otherwise their
    // gutters consume space, so the span and originating row differ in size.
    for i in 0..out.len() {
        for j in 0..out[i].cells.len() {
            let span = out[i].cells[j].span;
            if span <= 1 {
                continue;
            }
            let expandable = spacing.abs() < COORD_EPS
                && out[i + 1..i + span].iter().all(|r| {
                    r.height <= COORD_EPS && r.cells.iter().all(|c| c.shape.height <= COORD_EPS)
                });
            out[i].cells[j].expandable = expandable;
            if expandable {
                out[i].height = out[i].height.max(out[i].cells[j].shape.height);
            }
        }
    }
    // Keep the previous refusal for spans whose size distribution would
    // require a general table track sizing pass. Never invent break offsets.
    for (i, row) in out.iter().enumerate() {
        for cell in &row.cells {
            let span_height = out[i..i + cell.span].iter().map(|r| r.height).sum::<f32>()
                + spacing * (cell.span - 1) as f32;
            if cell.shape.height > span_height + COORD_EPS {
                return None;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    //! Regression checks for originating-row growth and row-group span limits.

    use super::row_geometry;
    use crate::dom::{Element, Node};
    use crate::layout::fragment::ShapeCx;
    use crate::layout::table::anon::anon_element;
    use crate::style::values::value::Len;

    fn block(height: f32) -> Node {
        let mut e = anon_element("div", Vec::new());
        e.style.height = Some(Len::Px(height));
        Node::Element(e)
    }

    fn cell(height: f32, span: &str) -> Node {
        let mut e = anon_element("td", vec![block(height)]);
        e.attrs.push(("rowspan".into(), span.into()));
        Node::Element(e)
    }

    fn row(children: Vec<Node>) -> Element {
        anon_element("tr", children)
    }

    #[test]
    fn clipped_span_uses_the_remaining_rows_in_its_group() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let a = row(vec![cell(170.0, "2")]);
        let b = row(vec![cell(20.0, "1")]);
        let g = row_geometry(&[(0, &a), (1, &b)], 4, ShapeCx::COLUMNS, 0.0, vcx).unwrap();
        assert_eq!(g[0].height, 170.0);
        assert_eq!(g[0].cells[0].span, 1);
        assert!(g[0].cells[0].expandable);
        assert_eq!(g[1].height, 20.0);
    }

    #[test]
    fn spanning_cell_growth_encompasses_only_its_originating_nonempty_row() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let a = row(vec![cell(180.0, "1"), cell(200.0, "2")]);
        let b = row(vec![cell(0.0, "1")]);
        let g = row_geometry(&[(0, &a), (0, &b)], 4, ShapeCx::COLUMNS, 0.0, vcx).unwrap();
        assert_eq!(g[0].height, 200.0);
        assert_eq!(g[1].height, 0.0);
        assert!(g[0].cells[1].expandable);
    }

    #[test]
    fn spanning_populated_rows_keeps_independent_track_sizes() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let a = row(vec![cell(180.0, "1"), cell(170.0, "0")]);
        let b = row(vec![cell(20.0, "1")]);
        let g = row_geometry(&[(0, &a), (0, &b)], 4, ShapeCx::COLUMNS, 0.0, vcx).unwrap();
        assert_eq!(g[0].height, 180.0);
        assert_eq!(g[1].height, 20.0);
        assert_eq!(g[0].cells[1].span, 2);
        assert!(!g[0].cells[1].expandable);
    }
}
