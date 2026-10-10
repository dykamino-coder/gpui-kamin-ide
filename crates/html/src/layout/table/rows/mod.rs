//! Цикл рядов таблицы: ячейки, рамки ячеек, фоны (table_rows).

use crate::dom::{Element, Node};
use crate::layout::table::{is_cell, table_finish};
use crate::render::{RenderOpts, gather_text};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;
mod row_setup;
use row_setup::hides_empty_cell;
use row_setup::{cell_cascaded_style, cell_spans, push_empty_row_track, row_band_probes};
mod cell_box;
use cell_box::prepare_cell_box;
mod cell_heights;
use cell_heights::resolve_cell_heights;
mod emit;
use emit::emit_cell;
mod cell_div;
mod cell_inside;
mod edge_probes;

#[allow(clippy::too_many_arguments, clippy::unnecessary_cast)]
pub(super) fn table_rows(
    rows: Vec<(
        &Element,
        (
            f32,
            f32,
            Option<crate::style::values::value::Color>,
            Option<&Element>,
        ),
    )>,
    mut row_ix: i16,
    mut occupied: Vec<u16>,
    opts: &RenderOpts,
    mut under: Vec<AnyElement>,
    group_of: std::collections::HashMap<u64, (&Element, bool, bool)>,
    inherited: &Computed,
    e: &Element,
    cols: u16,
    mut cells: Vec<AnyElement>,
    zero_cols: Vec<bool>,
    rows_left: Vec<usize>,
    cols_collapsed: Vec<bool>,
    collapse_cells: bool,
    win_edges: std::collections::HashMap<u64, [f32; 4]>,
    table_is_vertical: bool,
    table_font: f32,
    table_family: &String,
    paint_layers: bool,
    cell_bgs: std::rc::Rc<std::cell::RefCell<Vec<(gpui::Bounds<gpui::Pixels>, gpui::Hsla)>>>,
    row_elements: Vec<&Element>,
    table_edges: std::rc::Rc<std::cell::RefCell<Vec<crate::layout::table::paint::EdgeCell>>>,
    col_rects: Vec<Option<crate::layout::table::paint::RowRects>>,
    col_els: Vec<Option<&Element>>,
    grp_rects: Vec<Option<crate::layout::table::paint::RowRects>>,
    grp_els: Vec<Option<&Element>>,
    rules_groups: bool,
    mut group_refs: std::collections::HashMap<
        u64,
        crate::paint::effects::transformed_element::RefBox,
    >,
    tbl_style: &Computed,
    mut cells_over: Vec<AnyElement>,
    spacing: (f32, f32),
    from_cols: Vec<Option<f32>>,
    col_widths: Vec<(Option<f32>, Option<f32>)>,
    cols_pct: Vec<Option<f32>>,
    bw: [f32; 4],
    outer_win: [f32; 4],
    have_rows: bool,
    px_of: impl Fn(Option<Len>) -> f32,
) -> AnyElement {
    for (row, carry) in rows {
        row_ix += 1;
        let row_ref: crate::paint::effects::transformed_element::RefBox = Default::default();
        for slot in occupied.iter_mut() {
            *slot = slot.saturating_sub(1);
        }
        let (row_rects, grp_band) = row_band_probes(opts, &mut under, &group_of, row, carry);
        let shift = (carry.0, carry.1);
        // Письмо к строкам НЕ ПРИМЕНЯЕТСЯ (раскладку ряда ведёт таблица,
        // CSS Writing Modes §3.1) — но ВЫЧИСЛЕННОЕ значение наследуется в
        // ячейки как у любого свойства: `tr { writing-mode; line-height: 5ch }`
        // обязан дать ячейке вертикальное содержимое (ch-units-vrl-*).
        // Ряд у нас и так не строит своей коробки — урезать нечего.
        let own = row.style.clone();
        // Слой ГРУППЫ строк между таблицей и рядом: наследуемое с `<tbody>`
        // течёт вниз, как у любого предка.
        let group_layer;
        let inherited = match carry.3 {
            Some(g) => {
                group_layer = inherit(inherited, &g.style);
                &group_layer
            }
            None => inherited,
        };
        // Направление на строке ОСТАЁТСЯ: замерено, что его обнуление сдвигает
        // ячейки в парах `position-relative-table-*-left` (29 → 25).
        // ПРОБОВАЛИ ТРИЖДЫ И ОТКАТИЛИ: доводить до ячеек наследуемые свойства
        // САМОЙ таблицы (`inherit(inherited, &e.style)` как основа).
        // Дыра настоящая — `white-space` и шрифт с тега таблицы до ячейки не
        // доходят, — но цена: css-text −3 (`shaping-tatweel-002/003`,
        // `shaping-join-003`), а выигрыш НУЛЕВОЙ: семейство
        // `ws-break-spaces-applies-to` не двигается ни на пару. Значит
        // сохранённые пробелы в ячейке теряются НЕ здесь, и до того, как
        // найдено настоящее место, правка только вредит.
        // `inherited` — УЖЕ слитый стиль самой таблицы, поэтому второй мерж
        // сырого `e.style` разрешал относительные единицы повторно:
        // `font-size: 2em` на теге давал ячейке 64 точки вместо 32, строки
        // не влезали в колонку и таблица разъезжалась на лишние полосы
        // (вся семья `table-anonymous-objects-059…098`).
        let row_style = inherit(inherited, &own);
        push_empty_row_track(row_ix, &occupied, e, cols, &mut cells, row);
        let mut col_ix = 0usize;
        for child in &row.children {
            let Node::Element(cell) = child else { continue };
            if !is_cell(cell) {
                continue;
            }
            while col_ix < occupied.len() && occupied[col_ix] > 0 {
                col_ix += 1;
            }
            // §17.6.1.1: `empty-cells: hide` прячет фон и рамку ПУСТОЙ
            // ячейки — в раздельной модели рамок. Пустая это та, у которой нет
            // ни текста, ни элементов-детей.
            let прячем_пустую = hides_empty_cell(e, &row_style, cell);
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
            let cell = &if zero_cols.get(col_ix).copied().unwrap_or(false) {
                let mut copy = cell.clone();
                copy.style.padding.left = Some(Len::Px(0.0));
                copy.style.padding.right = Some(Len::Px(0.0));
                copy.style.border_width.left = Some(Len::Px(0.0));
                copy.style.border_width.right = Some(Len::Px(0.0));
                copy
            } else {
                cell.clone()
            };
            let mut cm = cell_cascaded_style(e, &row_style, cell);
            let (span_cols, span_rows, spans_collapsed, clipped) =
                cell_spans(row_ix, &rows_left, &cols_collapsed, col_ix, cell);
            let mut cell = cell.clone();
            let cell_edge = prepare_cell_box(
                e,
                &cols_collapsed,
                collapse_cells,
                &win_edges,
                table_is_vertical,
                table_font,
                table_family,
                &px_of,
                row,
                col_ix,
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
                &mut occupied,
                opts,
                &group_of,
                e,
                cols,
                &mut cells,
                collapse_cells,
                paint_layers,
                &cell_bgs,
                &row_elements,
                &table_edges,
                &col_rects,
                &col_els,
                &grp_rects,
                &grp_els,
                rules_groups,
                &mut group_refs,
                tbl_style,
                &mut cells_over,
                &px_of,
                row,
                carry,
                &row_ref,
                &row_rects,
                &grp_band,
                shift,
                inherited,
                &row_style,
                &mut col_ix,
                cm,
                span_cols,
                span_rows,
                spans_collapsed,
                clipped,
                cell_edge,
                cell,
            );
        }
    }

    // Порядок слоёв сетки: полосы дорожек/рядов → фоны ячеек → сросшиеся
    // кромки → коробки ячеек с содержимым (см. `under`, `cell_bgs`). Прежде
    // слой кромок шёл ПОСЛЕДНИМ и накрывал всё содержимое ячеек.
    table_finish(
        under,
        paint_layers,
        cell_bgs,
        cells,
        table_edges,
        cells_over,
        e,
        inherited,
        opts,
        row_elements,
        win_edges,
        table_font,
        table_family,
        spacing,
        cols,
        from_cols,
        col_widths,
        cols_collapsed,
        cols_pct,
        bw,
        outer_win,
        have_rows,
        px_of,
    )
}
