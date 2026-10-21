//! Контракты табличного конвейера: геометрия для чтения, слои, курсор и выход.

use super::paint::{CellBgs, CellEdges, RowRects};
use crate::dom::Element;
use crate::paint::effects::transformed_element::RefBox;
use crate::style::computed::Computed;
use crate::style::values::value::Color;
use gpui::AnyElement;
use std::collections::HashMap;

pub(super) struct TableGeom<'a> {
    pub e: &'a Element,
    pub inherited: &'a Computed,
    pub cols: u16,
    pub zero_cols: Vec<bool>,
    pub rows_left: Vec<usize>,
    pub cols_collapsed: Vec<bool>,
    pub collapse_cells: bool,
    pub win_edges: HashMap<u64, [f32; 4]>,
    pub table_is_vertical: bool,
    pub table_font: f32,
    pub table_em: f32,
    pub table_family: String,
    pub row_elements: Vec<&'a Element>,
    pub spacing: (f32, f32),
    pub from_cols: Vec<Option<f32>>,
    pub col_widths: Vec<(Option<f32>, Option<f32>)>,
    pub cols_pct: Vec<Option<f32>>,
    pub bw: [f32; 4],
    pub outer_win: [f32; 4],
    pub have_rows: bool,
}

pub(super) struct ColumnLayer<'a> {
    pub element: Option<&'a Element>,
    pub rects: Option<RowRects>,
}

#[derive(Clone, Copy)]
pub(super) struct GroupEdge<'a> {
    pub element: &'a Element,
    pub first: bool,
    pub last: bool,
}

pub(super) struct TableLayers<'a> {
    pub columns: Vec<ColumnLayer<'a>>,
    pub groups: Vec<ColumnLayer<'a>>,
    pub group_of: HashMap<u64, GroupEdge<'a>>,
    pub paint_layers: bool,
    pub rules_groups: bool,
}

pub(super) struct RowCursor {
    pub row_ix: i16,
    pub col_ix: usize,
    pub occupied: Vec<u16>,
    pub group_refs: HashMap<u64, RefBox>,
    pub group_bands: HashMap<u64, RowRects>,
}

#[derive(Default)]
pub(super) struct RowsOut {
    pub under: Vec<AnyElement>,
    pub cells: Vec<AnyElement>,
    pub cells_over: Vec<AnyElement>,
    pub cell_bgs: CellBgs,
    pub table_edges: CellEdges,
}

pub(super) struct RowState<'a> {
    pub row: &'a Element,
    pub carry: super::anon::RowCarry<'a>,
    pub row_ref: RefBox,
    pub row_rects: Option<RowRects>,
    pub grp_band: Option<RowRects>,
    pub inherited: &'a Computed,
    pub row_style: Computed,
}

pub(super) struct CellLayout {
    pub cell: Element,
    pub cm: Computed,
    pub span_cols: u16,
    pub span_rows: u16,
    pub spans_collapsed: bool,
    pub clipped: bool,
    pub cell_edge: Option<CellEdge>,
}

pub(super) struct CellEdge {
    pub widths: [f32; 4],
    pub colors: [Color; 4],
    pub styles: [u8; 4],
    pub doc_ix: u32,
}

impl TableGeom<'_> {
    pub fn px_of(
        &self,
        l: Option<crate::style::values::value::Len>,
        opts: &crate::layout::options::RenderOpts,
    ) -> f32 {
        crate::text::metrics::spacing_px(l, &self.table_family, self.table_em, opts.cx.vcx())
    }
}
