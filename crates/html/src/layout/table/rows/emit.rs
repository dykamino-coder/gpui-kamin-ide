//! Выпуск ячейки в сетку таблицы: сборка коробки, содержимого, проб кромок и обёрток transform.

use super::cell_div::{cell_box_div, ortho_cell_box};
use super::cell_inside::{cell_contents, place_cell_in_grid};
use super::edge_probes::{CellProbe, cell_edge_probes};
use crate::dom::Element;
use crate::layout::build_contracts::Effect;
use crate::layout::options::RenderOpts;
use crate::layout::table::cell_paints_over;
use gpui::{AnyElement, IntoElement, ParentElement};

use crate::layout::table::context::{
    CellLayout, RowCursor, RowState, RowsOut, TableGeom, TableLayers,
};

pub(super) fn emit_cell(
    geom: &TableGeom<'_>,
    layers: &TableLayers<'_>,
    cursor: &mut RowCursor,
    out: &mut RowsOut,
    opts: &RenderOpts,
    row_state: &RowState<'_>,
    prepared: CellLayout,
) {
    let CellLayout {
        cell,
        cm,
        span_cols,
        span_rows,
        spans_collapsed,
        clipped,
        cell_edge,
    } = prepared;
    let cell = &cell;
    let d = cell_box_div(layers, out, row_state, &cm, cell, &opts.cx);
    let mut d = ortho_cell_box(geom.e, row_state.row, &cm, cell, d);
    d = place_cell_in_grid(
        cursor.row_ix,
        geom.e,
        geom.cols,
        &geom.row_elements,
        cursor.col_ix,
        span_cols,
        span_rows,
        d,
    );
    for c in cursor.col_ix..(cursor.col_ix + span_cols as usize).min(cursor.occupied.len()) {
        cursor.occupied[c] = span_rows;
    }
    cursor.col_ix += span_cols as usize;
    let (inside, d) = cell_contents(opts, geom.e, cm, spans_collapsed, clipped, cell, d);
    let d = if crate::layout::table::anon::row_collapsed(row_state.row, row_state.carry.group) {
        d
    } else {
        cell_edge_probes(
            geom,
            layers,
            cursor,
            out,
            row_state,
            CellProbe {
                span_cols,
                span_rows,
                cell_edge,
                cell,
                d,
            },
            opts,
        )
    };
    // `transform` ячейки, ряда и группы рядов (css-transforms-1
    // §transformable-element: «table-row-group, table-header-group,
    // table-footer-group, table-row, table-column-group,
    // table-column, table-cell»). Своей коробки у ряда и группы в
    // сетке нет, поэтому их ПЕРЕНОС (не зависящий от точки отсчёта)
    // переходит на каждую ячейку; поворот/масштаб ряда требует его
    // коробки и пока не применяется.
    push_cell_wrapped(
        geom,
        layers,
        cursor,
        out,
        row_state,
        CellContent { cell, inside, d },
        opts,
    );
}

struct CellContent<'a> {
    cell: &'a Element,
    inside: Vec<AnyElement>,
    d: gpui::Div,
}

fn push_cell_wrapped(
    geom: &TableGeom<'_>,
    layers: &TableLayers<'_>,
    cursor: &mut RowCursor,
    out: &mut RowsOut,
    row_state: &RowState<'_>,
    content: CellContent<'_>,
    opts: &RenderOpts,
) {
    let CellContent { cell, inside, d } = content;
    let vcx = opts.cx.vcx();
    let mut built = d.children(inside).into_any_element();
    // Also without own transform: a `preserve-3d` cell or one under a
    // 3D row needs its wrapper for the context chain.
    let build = &opts.cx.build;
    built = build.effects(
        built,
        Effect::transform(&cell.style.computed(), &row_state.row_style, vcx),
    );
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
    // boxes, shared by their wrappers (`Effect::Transform::ref_box`;
    // `transform-transformed-tr-contains-fixed-position`: `rotate(45deg)`
    // with `transform-origin: left` on the `<tr>`).
    // The wrappers also carry a `preserve-3d` chain through the
    // row and group (css-transforms-2 §3d-rendering-context:
    // `transform-table-009/011`); without transform, perspective or
    // 3D context `transformed_with` returns the cell unchanged.
    let rb = row_state
        .row
        .style
        .computed()
        .transform
        .as_ref()
        .is_some_and(|t| !pure_shift(t))
        .then(|| row_state.row_ref.clone());
    built = build.effects(
        built,
        Effect::Transform {
            style: &row_state.row.style.computed(),
            parent: row_state.inherited,
            ref_box: rb,
            vcx,
        },
    );
    if let Some(g) = row_state.carry.group {
        let rb = g
            .style
            .computed()
            .transform
            .as_ref()
            .is_some_and(|t| !pure_shift(t))
            .then(|| cursor.group_refs.entry(g.node_id).or_default().clone());
        built = build.effects(
            built,
            Effect::Transform {
                style: &g.style.computed(),
                parent: geom.inherited,
                ref_box: rb,
                vcx,
            },
        );
    }
    if layers.paint_layers && cell_paints_over(&cell.children, 24) {
        out.cells_over.push(built);
    } else {
        out.cells.push(built);
    }
}
