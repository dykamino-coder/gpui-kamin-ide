//! Ключи повтора ряда таблицы и данные его проб (`super`): входы сборки
//! ряда вне его поддерева, названные поимённо, и то, что ячейки сообщают
//! таблице в prepaint.

use crate::layout::reuse::{Effects, Shallow};
use crate::layout::table::context::{TableGeom, TableLayers};
use crate::layout::table::probes::{CellBgs, CellEdges, EdgeCell};
use crate::style::values::value::Color;
use gpui::{Bounds, Hsla, Pixels, Point};
use std::any::Any;
use std::rc::Rc;

/// Inputs of every row of a table besides the flow key.
pub(super) struct TableKey {
    pub(super) table: Shallow,
    pub(super) columns: Vec<Option<Shallow>>,
    pub(super) groups: Vec<Option<Shallow>>,
    pub(super) geom: GeomKey,
}

#[derive(PartialEq)]
pub(super) struct GeomKey {
    cols: u16,
    zero_cols: Vec<bool>,
    cols_collapsed: Vec<bool>,
    collapse_cells: bool,
    table_font: f32,
    table_em: f32,
    table_family: String,
    spacing: (f32, f32),
    from_cols: Vec<Option<f32>>,
    col_widths: Vec<(Option<f32>, Option<f32>)>,
    cols_pct: Vec<Option<f32>>,
    bw: [f32; 4],
    outer_win: [f32; 4],
    have_rows: bool,
    paint_layers: bool,
    rules_groups: bool,
}

impl GeomKey {
    pub(super) fn of(geom: &TableGeom<'_>, layers: &TableLayers<'_>) -> Self {
        Self {
            cols: geom.cols,
            zero_cols: geom.zero_cols.clone(),
            cols_collapsed: geom.cols_collapsed.clone(),
            collapse_cells: geom.collapse_cells,
            table_font: geom.table_font,
            table_em: geom.table_em,
            table_family: geom.table_family.clone(),
            spacing: geom.spacing,
            from_cols: geom.from_cols.clone(),
            col_widths: geom.col_widths.clone(),
            cols_pct: geom.cols_pct.clone(),
            bw: geom.bw,
            outer_win: geom.outer_win,
            have_rows: geom.have_rows,
            paint_layers: layers.paint_layers,
            rules_groups: layers.rules_groups,
        }
    }
}

/// Inputs of one row besides its subtree.
#[derive(PartialEq)]
pub(in crate::layout::table::rows) struct RowKey {
    pub(super) row_ix: usize,
    pub(super) last: bool,
    pub(super) covered: bool,
    /// Column and column span of each cell.
    pub(super) slots: Vec<(usize, u16)>,
    /// Winning collapsed edges of each cell (`TableGeom::win_edges`).
    pub(super) edges: Vec<Option<[f32; 4]>>,
    /// The row group: its node and whether the row is its first and last.
    pub(super) group: Option<(u64, bool, bool)>,
    pub(super) shift: (f32, f32),
    pub(super) background: Option<Color>,
}

pub(super) struct Payload {
    pub(super) table: Rc<TableKey>,
    pub(super) row: RowKey,
    pub(super) group: Option<Shallow>,
}

/// The table's buffers the cells' probes push into in prepaint.
pub(super) struct TableEffects {
    pub(super) edges: CellEdges,
    pub(super) bgs: CellBgs,
}

type Kept = (Vec<EdgeCell>, Vec<(Bounds<Pixels>, Hsla)>);

fn moved(bounds: &Bounds<Pixels>, by: Point<Pixels>) -> Bounds<Pixels> {
    Bounds {
        origin: bounds.origin + by,
        size: bounds.size,
    }
}

impl Effects for TableEffects {
    fn mark(&self) -> [usize; 2] {
        [self.edges.borrow().len(), self.bgs.borrow().len()]
    }

    fn capture(&self, from: [usize; 2]) -> Rc<dyn Any> {
        let (edges, bgs) = (self.edges.borrow(), self.bgs.borrow());
        let kept: Kept = (
            edges.get(from[0]..).unwrap_or_default().to_vec(),
            bgs.get(from[1]..).unwrap_or_default().to_vec(),
        );
        Rc::new(kept)
    }

    fn replay(&self, data: &dyn Any, by: Point<Pixels>) {
        let Some((edges, bgs)) = data.downcast_ref::<Kept>() else {
            return;
        };
        for edge in edges {
            self.edges.push(EdgeCell {
                bounds: moved(&edge.bounds, by),
                ..edge.clone()
            });
        }
        for (bounds, colour) in bgs {
            self.bgs.push((moved(bounds, by), *colour));
        }
    }
}
