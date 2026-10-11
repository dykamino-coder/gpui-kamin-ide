//! Одна ячейка ряда в цикле table_rows: стиль, охваты, коробка, высоты и выпуск (table_cell).

use super::cell_box::prepare_cell_box;
use super::cell_heights::resolve_cell_heights;
use super::emit::emit_cell;
use super::row_setup::hides_empty_cell;
use super::row_setup::{cell_cascaded_style, cell_spans};
use crate::dom::Element;
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;

#[allow(clippy::too_many_arguments)]
pub(super) fn table_cell(
    row_ix: i16,
    occupied: &mut [u16],
    opts: &RenderOpts,
    group_of: &std::collections::HashMap<u64, (&Element, bool, bool)>,
    e: &Element,
    cols: u16,
    cells: &mut Vec<AnyElement>,
    zero_cols: &[bool],
    rows_left: &[usize],
    cols_collapsed: &[bool],
    collapse_cells: bool,
    win_edges: &std::collections::HashMap<u64, [f32; 4]>,
    table_is_vertical: bool,
    table_font: f32,
    table_family: &str,
    paint_layers: bool,
    cell_bgs: &std::rc::Rc<std::cell::RefCell<Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)>>>,
    row_elements: &Vec<&Element>,
    table_edges: &std::rc::Rc<std::cell::RefCell<Vec<super::super::paint::EdgeCell>>>,
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
    cell: &Element,
) {
    // §17.6.1.1: `empty-cells: hide` прячет фон и рамку ПУСТОЙ
    // ячейки — в раздельной модели рамок. Пустая это та, у которой нет
    // ни текста, ни элементов-детей.
    let прячем_пустую = hides_empty_cell(e, row_style, cell);
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
    let cell = &if zero_cols.get(*col_ix).copied().unwrap_or(false) {
        let mut copy = cell.clone();
        copy.style.padding.left = Some(Len::Px(0.0));
        copy.style.padding.right = Some(Len::Px(0.0));
        copy.style.border_width.left = Some(Len::Px(0.0));
        copy.style.border_width.right = Some(Len::Px(0.0));
        copy
    } else {
        cell.clone()
    };
    let mut cm = cell_cascaded_style(e, row_style, cell);
    let (span_cols, span_rows, spans_collapsed, clipped) =
        cell_spans(row_ix, rows_left, cols_collapsed, *col_ix, cell);
    let mut cell = cell.clone();
    let cell_edge = prepare_cell_box(
        e,
        cols_collapsed,
        collapse_cells,
        win_edges,
        table_is_vertical,
        table_font,
        table_family,
        px_of,
        row,
        *col_ix,
        &cm,
        span_cols,
        spans_collapsed,
        clipped,
        &mut cell,
    );
    resolve_cell_heights(opts, e, row, inherited, &mut cm, &mut cell);
    let cell = &cell;
    emit_cell(
        row_ix,
        occupied,
        opts,
        group_of,
        e,
        cols,
        cells,
        collapse_cells,
        paint_layers,
        cell_bgs,
        row_elements,
        table_edges,
        col_rects,
        col_els,
        grp_rects,
        grp_els,
        rules_groups,
        group_refs,
        tbl_style,
        cells_over,
        px_of,
        row,
        carry,
        row_ref,
        row_rects,
        grp_band,
        shift,
        inherited,
        row_style,
        col_ix,
        cm,
        span_cols,
        span_rows,
        spans_collapsed,
        clipped,
        cell_edge,
        cell,
    );
}
