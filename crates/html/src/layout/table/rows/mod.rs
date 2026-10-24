//! Цикл рядов таблицы: ячейки, рамки ячеек, фоны (table_rows).

use crate::dom::Node;
use crate::dom::text_content::gather_text;
use crate::layout::options::RenderOpts;
use crate::layout::table::{is_cell, table_finish};
use gpui::AnyElement;
mod row_setup;
use row_setup::{RowBands, push_empty_row_track, row_band_probes};
mod cell;
mod cell_alignment;
mod cell_box;
mod cell_columns;
mod cell_div;
mod cell_heights;
mod cell_inside;
mod cell_size;
mod edge_probes;
mod emit;
use cell::table_cell;

use super::context::{RowCursor, RowState, RowsOut, TableGeom, TableLayers};
use crate::style::cascade::materialize::box_style;

pub(super) fn table_rows(
    rows: &[super::anon::TableRow<'_>],
    geom: &TableGeom<'_>,
    layers: &TableLayers<'_>,
    cursor: &mut RowCursor,
    out: &mut RowsOut,
    opts: &RenderOpts,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    // Буфер полосы группы рядов — общий для всех её рядов (`RowRects`).
    for record in rows {
        let row = record.element;
        let carry = record.carry;
        cursor.row_ix += 1;
        let row_ref: crate::layout::build_contracts::RefBox = Default::default();
        for slot in cursor.occupied.iter_mut() {
            *slot = slot.saturating_sub(1);
        }
        let RowBands {
            row_rects,
            grp_band,
        } = row_band_probes(
            &mut cursor.group_bands,
            &mut out.under,
            &layers.group_of,
            row,
            carry,
            &opts.cx,
        );
        // Письмо к строкам НЕ ПРИМЕНЯЕТСЯ (раскладку ряда ведёт таблица,
        // CSS Writing Modes §3.1) — но ВЫЧИСЛЕННОЕ значение наследуется в
        // ячейки как у любого свойства: `tr { writing-mode; line-height: 5ch }`
        // обязан дать ячейке вертикальное содержимое (ch-units-vrl-*).
        // Ряд у нас и так не строит своей коробки — урезать нечего.
        let own = row.style.clone();
        // Слой ГРУППЫ строк между таблицей и рядом: наследуемое с `<tbody>`
        // течёт вниз, как у любого предка.
        let group_layer;
        let inherited = match carry.group {
            Some(g) => {
                group_layer = box_style(geom.inherited, &g.style, vcx);
                &group_layer
            }
            None => geom.inherited,
        };
        // Направление на строке ОСТАЁТСЯ: его обнуление сдвигает ячейки в
        // `position-relative-table-*-left`.
        // Не доводить до ячеек наследуемые свойства САМОЙ таблицы
        // (`inherit(geom.inherited, &e.style)` как основа). Дыра настоящая —
        // `white-space` и шрифт с тега таблицы до ячейки не доходят, — но
        // `ws-break-spaces-applies-to` это не чинит (сохранённые пробелы в
        // ячейке теряются НЕ здесь), а css-text ломает
        // (`shaping-tatweel-002/003`, `shaping-join-003`).
        // `inherited` — УЖЕ слитый стиль самой таблицы, поэтому второй мерж
        // сырого `e.style` разрешал относительные единицы повторно:
        // `font-size: 2em` на теге давал ячейке 64 точки вместо 32, строки
        // не влезали в колонку и таблица разъезжалась на лишние полосы
        // (вся семья `table-anonymous-objects-059…098`).
        let row_style = crate::style::cascade::materialize::box_style(inherited, &own, vcx);
        push_empty_row_track(
            cursor.row_ix,
            &cursor.occupied,
            geom.e,
            geom.cols,
            &mut out.cells,
            row,
        );
        cursor.col_ix = 0;
        let row_state = RowState {
            row,
            carry,
            row_ref,
            row_rects,
            grp_band,
            inherited,
            row_style,
        };
        for child in &row.children {
            let Node::Element(cell) = child else { continue };
            if !is_cell(cell) {
                continue;
            }
            while cursor.col_ix < cursor.occupied.len() && cursor.occupied[cursor.col_ix] > 0 {
                cursor.col_ix += 1;
            }
            table_cell(geom, layers, cursor, out, opts, &row_state, cell);
        }
    }

    // Порядок слоёв сетки: полосы дорожек/рядов → фоны ячеек → сросшиеся
    // кромки → коробки ячеек с содержимым (см. `under`, `cell_bgs`). Прежде
    // слой кромок шёл ПОСЛЕДНИМ и накрывал всё содержимое ячеек.
    table_finish(geom, layers, out, opts)
}
