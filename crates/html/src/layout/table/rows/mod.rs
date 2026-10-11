//! Цикл рядов таблицы: ячейки, рамки ячеек, фоны (table_rows).

use crate::dom::{Element, Node};
use crate::layout::table::{is_cell, table_finish};
use crate::render::{RenderOpts, gather_text};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::AnyElement;
mod row_setup;
use row_setup::{push_empty_row_track, row_band_probes};
mod cell;
mod cell_box;
mod cell_columns;
mod cell_div;
mod cell_heights;
mod cell_inside;
mod edge_probes;
mod emit;
use cell::table_cell;

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
            table_cell(
                row_ix,
                &mut occupied,
                opts,
                &group_of,
                e,
                cols,
                &mut cells,
                &zero_cols,
                &rows_left,
                &cols_collapsed,
                collapse_cells,
                &win_edges,
                table_is_vertical,
                table_font,
                table_family,
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
