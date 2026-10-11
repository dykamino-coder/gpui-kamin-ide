//! Выпуск ячейки в сетку таблицы: сборка коробки, содержимого, проб кромок и обёрток transform.

use super::cell_div::{cell_box_div, ortho_cell_box};
use super::cell_inside::{cell_contents, place_cell_in_grid};
use super::edge_probes::cell_edge_probes;
use crate::dom::Element;
use crate::layout::table::cell_paints_over;
use crate::paint::effects::transform::{transformed, transformed_with};
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_cell(
    row_ix: i16,
    occupied: &mut [u16],
    opts: &RenderOpts,
    group_of: &std::collections::HashMap<u64, (&Element, bool, bool)>,
    e: &Element,
    cols: u16,
    cells: &mut Vec<AnyElement>,
    collapse_cells: bool,
    paint_layers: bool,
    cell_bgs: &std::rc::Rc<std::cell::RefCell<Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)>>>,
    row_elements: &Vec<&Element>,
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    col_rects: &[Option<crate::layout::table::paint::RowRects>],
    col_els: &Vec<Option<&Element>>,
    grp_rects: &[Option<crate::layout::table::paint::RowRects>],
    grp_els: &Vec<Option<&Element>>,
    rules_groups: bool,
    group_refs: &mut std::collections::HashMap<
        u64,
        crate::paint::effects::transformed_element::RefBox,
    >,
    tbl_style: &Computed,
    cells_over: &mut Vec<AnyElement>,
    px_of: &impl Fn(Option<Len>) -> f32,
    row: &Element,
    carry: (
        f32,
        f32,
        Option<crate::style::values::value::Color>,
        Option<&Element>,
    ),
    row_ref: &crate::paint::effects::transformed_element::RefBox,
    row_rects: &Option<crate::layout::table::paint::RowRects>,
    grp_band: &Option<crate::layout::table::paint::RowRects>,
    shift: (f32, f32),
    inherited: &Computed,
    row_style: &Computed,
    col_ix: &mut usize,
    cm: Computed,
    span_cols: u16,
    span_rows: u16,
    spans_collapsed: bool,
    clipped: bool,
    cell_edge: Option<(
        [f32; 4],
        [crate::style::values::value::Color; 4],
        [u8; 4],
        u32,
    )>,
    cell: &Element,
) {
    let d = cell_box_div(paint_layers, cell_bgs, row, carry, shift, &cm, cell);
    let mut d = ortho_cell_box(e, row, &cm, cell, d);
    d = place_cell_in_grid(
        row_ix,
        e,
        cols,
        row_elements,
        *col_ix,
        span_cols,
        span_rows,
        d,
    );
    for c in *col_ix..(*col_ix + span_cols as usize).min(occupied.len()) {
        occupied[c] = span_rows;
    }
    *col_ix += span_cols as usize;
    let (inside, d) = cell_contents(opts, e, cm, spans_collapsed, clipped, cell, d);
    let d = cell_edge_probes(
        row_ix,
        group_of,
        cols,
        collapse_cells,
        row_elements,
        table_edges,
        col_rects,
        col_els,
        grp_rects,
        grp_els,
        rules_groups,
        px_of,
        row,
        row_rects,
        grp_band,
        inherited,
        row_style,
        *col_ix,
        span_cols,
        span_rows,
        cell_edge,
        cell,
        d,
    );
    // `transform` ячейки, ряда и группы рядов (css-transforms-1
    // §transformable-element: «table-row-group, table-header-group,
    // table-footer-group, table-row, table-column-group,
    // table-column, table-cell»). Своей коробки у ряда и группы в
    // сетке нет, поэтому их ПЕРЕНОС (не зависящий от точки отсчёта)
    // переходит на каждую ячейку; поворот/масштаб ряда требует его
    // коробки и пока не применяется.
    push_cell_wrapped(
        cells,
        paint_layers,
        group_refs,
        tbl_style,
        cells_over,
        row,
        carry,
        row_ref,
        inherited,
        row_style,
        cell,
        inside,
        d,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn push_cell_wrapped(
    cells: &mut Vec<AnyElement>,
    paint_layers: bool,
    group_refs: &mut std::collections::HashMap<
        u64,
        crate::paint::effects::transformed_element::RefBox,
    >,
    tbl_style: &Computed,
    cells_over: &mut Vec<AnyElement>,
    row: &Element,
    carry: (
        f32,
        f32,
        Option<crate::style::values::value::Color>,
        Option<&Element>,
    ),
    row_ref: &crate::paint::effects::transformed_element::RefBox,
    inherited: &Computed,
    row_style: &Computed,
    cell: &Element,
    inside: Vec<AnyElement>,
    d: gpui::Div,
) {
    let mut built = d.children(inside).into_any_element();
    // Also without own transform: a `preserve-3d` cell or one under a
    // 3D row needs its wrapper for the context chain.
    built = transformed(built, &cell.style, row_style);
    let pure_shift = |t: &crate::style::computed::Transform| {
        !t.has_3d
            && t.lin == [[1.0, 0.0], [0.0, 1.0]]
            && t.tr[0][1] == 0.0
            && t.tr[0][2] == 0.0
            && t.tr[1][1] == 0.0
            && t.tr[1][2] == 0.0
    };
    // Any other row/group transform resolves its origin and
    // percentages against the row/group box: the union of its cells'
    // boxes, shared by their wrappers (`interact::Transformed::ref_box`;
    // `transform-transformed-tr-contains-fixed-position`: `rotate(45deg)`
    // with `transform-origin: left` on the `<tr>`).
    // The wrappers also carry a `preserve-3d` chain through the
    // row and group (css-transforms-2 §3d-rendering-context:
    // `transform-table-009/011`); without transform, perspective or
    // 3D context `transformed_with` returns the cell unchanged.
    let rb = row
        .style
        .transform
        .as_ref()
        .is_some_and(|t| !pure_shift(t))
        .then(|| row_ref.clone());
    built = transformed_with(built, &row.style, inherited, rb);
    if let Some(g) = carry.3 {
        let rb = g
            .style
            .transform
            .as_ref()
            .is_some_and(|t| !pure_shift(t))
            .then(|| group_refs.entry(g.node_id).or_default().clone());
        built = transformed_with(built, &g.style, tbl_style, rb);
    }
    if paint_layers && cell_paints_over(&cell.children, 24) {
        cells_over.push(built);
    } else {
        cells.push(built);
    }
}
