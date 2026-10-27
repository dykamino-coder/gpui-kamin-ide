//! Форма коробки таблицы без рядов при фрагментации: рамка, отбивка, заданная высота, подписи.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
mod captions;
pub(super) use captions::caption_slots_shape;

/// Коробка таблицы без рядов: её высота — рамка с отбивкой, вертикальный
/// `border-spacing` и пол заданной высоты; ряды таблицы с рядами кладёт и
/// измеряет только раскладка (`flow::blocks::table`).
#[allow(clippy::too_many_arguments)]
pub(super) fn row_band_shape(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
    spec_h: Option<f32>,
    spec_min_h: Option<f32>,
    mt: f32,
    mb: f32,
    top: f32,
    bot: f32,
    spacing: f32,
    caps_top: Vec<&Element>,
    caps_bot: Vec<&Element>,
    vcx: ValueCx<'_>,
) -> Option<crate::layout::fragment::Shape> {
    let mut solid: Vec<(f32, f32)> = Vec::new();
    if top > 0.0 {
        solid.push((0.0, top));
    }
    // Заданная высота — ПОЛ коробки рядов. `box-sizing` — та же мерка, что в
    // рисователе (`table()`, `table_border_box`): у ТЕГА `<table>` высота по
    // border-box (UA-правило css-tables-3 `table { box-sizing: border-box }`),
    // у `display: table` на прочих тегах — контентная. `min-height` мерится
    // ПОЛНОЙ коробкой ВСЕГДА — так его кладёт `min_fix` в `table()`
    // (css-tables-3 §computing-the-table-height, CSSWG #5336).
    let content_h = top + spacing + bot;
    let edges = top + bot;
    let border_box = c.style.used().border_box == Some(true)
        || (c.tag == "table" && c.style.used().border_box.is_none());
    let floor_h = spec_h
        .map(|v| if border_box { v.max(edges) } else { v + edges })
        .into_iter()
        .chain(spec_min_h.map(|v| v.max(edges)))
        .fold(0.0f32, f32::max);
    let h_box = content_h.max(floor_h);
    if bot > 0.0 {
        // Нижняя рамка/отбивка таблицы — сплошная (css-break-4
        // §possible-breaks: класс C только при ненулевом зазоре).
        solid.push((h_box - bot, h_box));
    }
    // Подписей нет — коробка и есть вся мера.
    if caps_top.is_empty() && caps_bot.is_empty() {
        return Some(crate::layout::fragment::Shape {
            height: h_box,
            margin_top: mt,
            margin_bottom: mb,
            cuts: Vec::new(),
            forced: Vec::new(),
            solid,
        });
    }
    // Обёртка таблицы — обычная блочная стопка: верхние подписи, коробка
    // таблицы, нижние подписи (css-tables-3 §terminology; Blink
    // `table_layout_algorithm.cc` «Add all the top captions» / «Add all the
    // bottom captions»). Между соседями обёртки — точка класса A, поля
    // соседей схлопываются; поля коробки таблицы несёт обёртка (`mt`/`mb`).
    caption_slots_shape(depth, cx, mt, mb, caps_top, caps_bot, solid, h_box, vcx)
}
