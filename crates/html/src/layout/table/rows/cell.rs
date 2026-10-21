//! Одна ячейка ряда в цикле table_rows: стиль, охваты, коробка, высоты и выпуск (table_cell).

use super::cell_box::prepare_cell_box;
use super::cell_heights::resolve_cell_heights;
use super::emit::emit_cell;
use super::row_setup::hides_empty_cell;
use super::row_setup::{cell_cascaded_style, cell_spans};
use crate::dom::Element;
use crate::layout::options::RenderOpts;
use crate::style::values::value::Len;

use crate::layout::table::context::{
    CellLayout, RowCursor, RowState, RowsOut, TableGeom, TableLayers,
};

pub(super) fn table_cell(
    geom: &TableGeom<'_>,
    layers: &TableLayers<'_>,
    cursor: &mut RowCursor,
    out: &mut RowsOut,
    opts: &RenderOpts,
    row_state: &RowState<'_>,
    cell: &Element,
) {
    let vcx = opts.cx.vcx();
    // §17.6.1.1: `empty-cells: hide` прячет фон и рамку ПУСТОЙ
    // ячейки — в раздельной модели рамок. Пустая это та, у которой нет
    // ни текста, ни элементов-детей.
    let прячем_пустую = hides_empty_cell(geom.e, &row_state.row_style, cell, vcx);
    // Ячейка в НУЛЕВОЙ дорожке: свои горизонтальные отступ и рамку
    // она держать не может — дорожки под них нет (§17.5.2.1).
    let cell = &if прячем_пустую {
        let mut copy = cell.clone();
        copy.style.background = None;
        copy.style.gradient = None;
        copy.style.bg_image = None;
        copy.style.border_visible = [Some(false); 4];
        copy.style.border_width = Default::default();
        copy
    } else {
        cell.clone()
    };
    let cell = &if geom.zero_cols.get(cursor.col_ix).copied().unwrap_or(false) {
        let mut copy = cell.clone();
        if geom.table_is_vertical {
            copy.style.padding.top = Some(Len::Px(0.0));
            copy.style.padding.bottom = Some(Len::Px(0.0));
            copy.style.border_width.top = Some(Len::Px(0.0));
            copy.style.border_width.bottom = Some(Len::Px(0.0));
        } else {
            copy.style.padding.left = Some(Len::Px(0.0));
            copy.style.padding.right = Some(Len::Px(0.0));
            copy.style.border_width.left = Some(Len::Px(0.0));
            copy.style.border_width.right = Some(Len::Px(0.0));
        }
        copy
    } else {
        cell.clone()
    };
    let mut cm = cell_cascaded_style(geom.e, &row_state.row_style, cell, vcx);
    let spans = cell_spans(
        cursor.row_ix,
        &geom.rows_left,
        &geom.cols_collapsed,
        cursor.col_ix,
        cell,
    );
    let mut cell = cell.clone();
    let cell_edge = prepare_cell_box(geom, row_state, cursor, &cm, &spans, &mut cell, opts);
    resolve_cell_heights(
        opts,
        geom.e,
        row_state.row,
        row_state.inherited,
        &mut cm,
        &mut cell,
    );
    emit_cell(
        geom,
        layers,
        cursor,
        out,
        opts,
        row_state,
        CellLayout {
            cell,
            cm,
            span_cols: spans.span_cols,
            span_rows: spans.span_rows,
            spans_collapsed: spans.spans_collapsed,
            clipped: spans.clipped,
            cell_edge,
        },
    );
}
