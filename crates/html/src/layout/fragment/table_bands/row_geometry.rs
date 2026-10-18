//! Shared row/cell geometry for table measurement and continuation probes.
//! Following CSS Tables 3 fragmentation, a spanning cell can expand its row
//! when row and span sizes match. Blink applies this encompassing check in
//! table_row_layout_algorithm.cc:112-118; spans over other non-empty
//! rows keep their independently sized row tracks.

use crate::dom::{Element, Node};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::table::is_cell;
use crate::render::is_blank;
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
            let shape = shape_full(cell, depth.checked_sub(1)?, cx)?;
            if span == 1 {
                height = height.max(shape.0);
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
                    r.height <= COORD_EPS && r.cells.iter().all(|c| c.shape.0 <= COORD_EPS)
                });
            out[i].cells[j].expandable = expandable;
            if expandable {
                out[i].height = out[i].height.max(out[i].cells[j].shape.0);
            }
        }
    }
    // Keep the previous refusal for spans whose size distribution would
    // require a general table track sizing pass. Never invent break offsets.
    for (i, row) in out.iter().enumerate() {
        for cell in &row.cells {
            let span_height = out[i..i + cell.span].iter().map(|r| r.height).sum::<f32>()
                + spacing * (cell.span - 1) as f32;
            if cell.shape.0 > span_height + COORD_EPS {
                return None;
            }
        }
    }
    Some(out)
}

#[cfg(test)]
#[path = "row_geometry_tests.rs"]
mod tests;
