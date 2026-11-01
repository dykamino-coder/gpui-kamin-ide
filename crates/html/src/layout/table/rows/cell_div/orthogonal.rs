//! Ортогональное письмо и выравнивание содержимого ячейки.
use crate::dom::Element;
use crate::layout::table::anon::html_cell;
use crate::style::computed::{Align, Computed};
use gpui::Styled;

pub(crate) fn ortho_cell_box(
    e: &Element,
    row: &Element,
    cm: &Computed,
    cell: &Element,
    d: gpui::Div,
) -> gpui::Div {
    // Умолчание браузера для ячейки — `vertical-align: middle`: без
    // него полоса высотой 10px в строке 22px стояла на 6 точек выше.
    // Ортогональная ячейка заводит СВОЙ контекст форматирования и
    // раскладывает содержимое в СВОЁМ письме (css-writing-modes-4
    // §3.1: `writing-mode` применяется к `table-cell`; §7.1: правила
    // горизонтальной оси переходят на вертикальную). Значит ось
    // блочного потока внутри неё ГОРИЗОНТАЛЬНА: дети идут справа
    // налево у `vertical-rl`/`sideways-rl` и слева направо у
    // `vertical-lr`/`sideways-lr`, а не столбиком. Тот же приём, что
    // у голого блока в `element()` (:14320). Blink строит место
    // ячейки её собственным письмом —
    // `table_layout_utils.cc:218`:
    // `ConstraintSpaceBuilder(table_writing_direction.GetWritingMode(),
    //                         cell_writing_direction, /* is_new_fc */ true)`.
    let ortho_cell = cm.wm().vertical() && !e.style.used().wm().vertical();
    let mut d = if ortho_cell {
        let d = d.flex();
        if cm.wm().block_rl() {
            d.flex_row_reverse()
        } else {
            d.flex_row()
        }
    } else {
        d.flex().flex_col()
    };
    if ortho_cell {
        // ОРТОГОНАЛЬНАЯ ячейка (вертикальный контент в горизонтальной
        // таблице): строчная ось вертикальна — `text-align` правит
        // ВЕРТИКАЛЬНОЕ положение строки (line-left = верх), а
        // `vertical-align` уходит на поперечную ось
        // (table-cell-align-005/006).
        use crate::style::computed::TextAlign;
        // `start`/`end` — края СТРОКИ: вертикальная строка идёт
        // сверху вниз, `dir=rtl` разворачивает её снизу вверх.
        let rtl = cm.wm().rtl();
        d = match cm.inh.text_flow.text_align {
            Some(TextAlign::Right) => d.items_end(),
            Some(TextAlign::Center) => d.items_center(),
            Some(TextAlign::End) if !rtl => d.items_end(),
            Some(TextAlign::Start) if rtl => d.items_end(),
            _ => d.items_start(),
        };
        // Начальное значение `vertical-align` — `baseline` (§17.5.3),
        // и у одиночного ряда это ВЕРХ ячейки, а не середина. Пока
        // умолчанием стояла середина, содержимое опускалось на
        // полразницы высот (`direction-applies-to-005`: квадрат на
        // 30 точек ниже эталона).
        d = match cm.inh.text_flow.vertical_align {
            Some(Align::End) => d.justify_end(),
            Some(Align::Center) => d.justify_center(),
            _ => d.justify_start(),
        };
    } else {
        d = match cm.inh.text_flow.vertical_align {
            Some(Align::End) => d.justify_end(),
            Some(Align::Center) => d.justify_center(),
            _ => d.justify_start(),
        };
        // `vertical-align: baseline` (CSS 2.1 §17.5.3): первые
        // базовые ячеек ряда совпадают, ряд растёт на сдвиг, а
        // коробка ячейки по-прежнему заполняет ряд — сдвигается
        // только содержимое (taffy `Style::table_cell_baseline`;
        // Blink `table_layout_utils.cc` `ComputeRowBaseline`).
        // Значение — СОБСТВЕННОЕ ячейки: `vertical-align` не
        // наследуется, а слитый `cm` тянет его от любого предка.
        // Только `td`/`th` берут значение ряда (UA-правило
        // `td, th { vertical-align: inherit }`).
        let own_va = cell.style.own_vertical_align().or(if html_cell(cell) {
            row.style.own_vertical_align()
        } else {
            None
        });
        if own_va == Some(Align::Baseline) && !e.style.used().wm().vertical() {
            d.style().table_cell_baseline = Some(true);
        }
    }
    super::super::cell_alignment::apply(cm, d)
}
