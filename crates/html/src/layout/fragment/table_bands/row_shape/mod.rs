//! Форма рядов таблицы при фрагментации: вырезы между рядами, подписи.

use super::{RowRef, TableBands, table_row_cuts};
use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::style::values::value::Len;
mod captions;
pub(super) use captions::caption_slots_shape;

#[allow(clippy::too_many_arguments)]
pub(super) fn row_band_shape(
    c: &Element,
    depth: u8,
    cx: ShapeCx,
    bands: &mut TableBands,
    px_of: impl Fn(&Option<Len>) -> Option<f32>,
    spec_h: Option<f32>,
    spec_min_h: Option<f32>,
    mt: f32,
    mb: f32,
    top: f32,
    bot: f32,
    spacing: f32,
    cell_cx: ShapeCx,
    caps_top: Vec<&Element>,
    caps_bot: Vec<&Element>,
    parts: Vec<(u8, &Element)>,
    rows: Vec<RowRef<'_>>,
) -> Option<(f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)> {
    let mut cuts: Vec<(f32, f32)> = Vec::new();
    let mut forced: Vec<f32> = Vec::new();
    let mut solid: Vec<(f32, f32)> = Vec::new();
    if top > 0.0 {
        solid.push((0.0, top));
    }
    let mut y = top;
    let mut group_open: Option<(usize, f32)> = None;
    // Сцепка рядов, скованных `break-before/after: avoid*`: начало открытого
    // диапазона, `open` предыдущего ряда и его `break-after: avoid*`.
    let mut avoid_run: Option<f32> = None;
    let prev_open = 0.0f32;
    let prev_aa = false;
    let force_next = false;
    // Ячейки с `rowspan`: (первый ряд, охват, высота содержимого). Их высота
    // НЕ растит свой ряд — она ложится на все охваченные (css-tables-3
    // §height-distribution); мера принимается, только если охват и так
    // вмещает ячейку (сверка после цикла), иначе — прежний отказ. Точки
    // разреза внутри такой ячейки не берутся: где именно внутри охвата
    // стоит её содержимое, мера не знает. Прежде любой `rowspan` отменял
    // меру таблицы целиком, и стол не фрагментировался вовсе
    // (`table-rowspan-001`: пустая ячейка `rowspan=2`, снимок — вторая
    // колонка пуста, стол переполняет первую).
    let mut spans: Vec<(usize, usize, f32)> = Vec::new();
    let mut row_box: Vec<(f32, f32)> = Vec::new();
    table_row_cuts(
        depth,
        bands,
        px_of,
        spacing,
        cell_cx,
        parts,
        rows,
        &mut cuts,
        &mut forced,
        &mut solid,
        &mut y,
        &mut group_open,
        &mut avoid_run,
        prev_open,
        prev_aa,
        force_next,
        &mut spans,
        &mut row_box,
    )?;
    bands.spacing = spacing;
    // Сверка охватов (см. `spans`): ячейка выше суммы своих рядов с зазорами
    // раздала бы им высоту — этого мера не умеет, отказ как прежде.
    for (i, rs, ch) in spans {
        let last = (i + rs).min(row_box.len()).saturating_sub(1);
        let span_h = row_box[last].0 + row_box[last].1 - row_box[i].0;
        if ch > span_h + 0.01 {
            return None;
        }
    }
    // Сцепка, дожившая до конца коробки рядов, закрывается её низом.
    if let Some(s) = avoid_run {
        solid.push((s, y));
    }
    if let Some((_, gs)) = group_open {
        solid.push((gs, y));
    }
    // Монолит ВНУТРИ коробки рядов (ячейка с `contain: size`, `break-inside:
    // avoid` ряда или группы) вместе с заданной высотой — прежний отказ.
    // Устройства «монолит переполняет колонку» у стопки колонок нет:
    // `flow.rs:826` при `holds(a, b)` и `a <= from` разреза не берёт, а
    // следующая ветка режет по краю колонки прямо сквозь монолит. Проба
    // `target/probe-ftb/p-mo-003.html` — та же геометрия ОДНИМИ блоками
    // (коробка 200 с двумя `contain: size` по 100 в колонках по 60) — даёт
    // «красное видно», то есть даже верная мера рисунка не спасает, а
    // `monolithic-overflow-003` сегодня 2.08: мера сделала бы ЕЙ ХУЖЕ.
    // Единственный диапазон, которого гейт не считает, — верхняя рамка
    // (0, top): её положили ДО цикла рядов.
    // Страницам этот отказ не нужен: у стопки листов переполнение монолитом
    // своё (`ColumnStack::fill_at`, ветка `paged && placed && cur > target`,
    // crbug 1402540), а «монолитом» здесь оказывается уже верхняя рамка любой
    // ячейки (`shape_full` кладёт её сплошным диапазоном — Blink
    // `FinishFragmentation`: «Avoid breaking inside block-start border»).
    // Отказ уводил таблицу с `block-size` в меру стопки блоков по тегу, где
    // высота считается content-box: `table-fragmentation-001b-print` —
    // 336 + отбивка + рамка = 432 вместо 336 по border-box (UA-лист
    // css-tables-3 `table { box-sizing: border-box }`), третий пустой лист.
    let mono_inside = solid.iter().any(|&(a, b)| a > 0.01 || b > top + 0.01);
    if mono_inside && !cx.paged && (spec_h.is_some() || spec_min_h.is_some()) {
        return None;
    }
    // Заданная высота — ПОЛ коробки рядов. `box-sizing` — та же мерка, что в
    // рисователе (`table()`, `table_border_box`): у ТЕГА `<table>` высота по
    // border-box (UA-правило css-tables-3 `table { box-sizing: border-box }`),
    // у `display: table` на прочих тегах — контентная. `min-height` мерится
    // ПОЛНОЙ коробкой ВСЕГДА — так его кладёт `min_fix` в `table()`
    // (css-tables-3 §computing-the-table-height, CSSWG #5336).
    let content_h = y + spacing + bot;
    let edges = top + bot;
    let border_box =
        c.style.border_box == Some(true) || (c.tag == "table" && c.style.border_box.is_none());
    let floor_h = spec_h
        .map(|v| if border_box { v.max(edges) } else { v + edges })
        .into_iter()
        .chain(spec_min_h.map(|v| v.max(edges)))
        .fold(0.0f32, f32::max);
    let h_box = content_h.max(floor_h);
    // Растянутая коробка раздаёт остаток РЯДАМ (CSS 2.1 §17.5.3; css-tables-3
    // §height-distribution-algorithm), и границы рядов уезжают с измеренных
    // мест: точки класса A между ними больше не верны. Такая коробка идёт
    // сплошным блоком — срез по краю колонки есть правило, а не исключение
    // (css-break-4 §4 «slice»). Снимок `specified-block-size-007`: жёлтый ряд
    // (10,67)..(172,316), голубой (10,317)..(172,566) — раздача у нас РОВНАЯ
    // (200/200) при содержимом 1 и 3, и точки на 1 и 4 были бы ложью.
    if h_box > content_h + 0.01 {
        // Ряды растянуты — измеренные полосы секций тоже неверны.
        bands.head = None;
        bands.foot = None;
        cuts.clear();
        forced.clear();
        solid.clear();
        if top > 0.0 {
            solid.push((0.0, top));
        }
    }
    if bot > 0.0 {
        // Нижняя рамка/отбивка таблицы приклеена к монолиту последнего ряда —
        // то же правило, что у блока (`shape_full`, Р4 break-rest): точки
        // разрыва перед block-end рамкой нет (css-break-4 §possible-breaks, класс
        // C — только при ненулевом зазоре; Blink `FinishFragmentation` держит там
        // лишь «last-resort breakpoint»). `table-border-006`: ряды `avoid` 100 и
        // 70, `border-bottom: 30px` в колонке 170 — рамка уходит вместе с
        // последним рядом, а не одна во вторую колонку. Зазор `border-spacing`
        // между рядом и рамкой — та же «без промежутка» граница: рамка таблицы
        // от ряда отделена именно им, а не полем.
        let end_edge = h_box - bot;
        let glue = solid
            .iter()
            .filter(|&&(a, b)| a > 0.01 && (b - (end_edge - spacing)).abs() < 0.01)
            .map(|&(a, _)| a)
            .fold(end_edge, f32::min);
        solid.push((glue, h_box));
    }
    bands.box_top = 0.0;
    bands.box_end = h_box;
    // Подписей нет — коробка рядов и есть вся мера, как прежде.
    if caps_top.is_empty() && caps_bot.is_empty() {
        cuts.retain(|&(need, _)| need > 0.01 && need < h_box - 0.01);
        forced.retain(|&f| f > 0.01 && f < h_box - 0.01);
        return Some((h_box, mt, mb, cuts, forced, solid));
    }
    // Обёртка таблицы — обычная блочная стопка: верхние подписи, коробка
    // рядов, нижние подписи (css-tables-3 §terminology; Blink
    // `table_layout_algorithm.cc:988` «Add all the top captions» и `:1584`
    // «Add all the bottom captions» — обе петли по ВСЕМ подписям своей
    // стороны, секции между ними). Между соседями обёртки — точка класса A
    // (css-break-4 §possible-breaks: «Between sibling boxes of the following
    // types: … in-flow block-level boxes»), поля соседей схлопываются, как в
    // блочной стопке `shape_full`, и подпись поля ИМЕЕТ (Blink `:1195`
    // «Captions allow margins»). Рамка и отбивка самой таблицы подпись не
    // трогают: она вне коробки рядов.
    // `None` в списке — сама коробка рядов; поля у неё НУЛЕВЫЕ, поля таблицы
    // носит обёртка (они уже в `mt`/`mb` и возвращаются наружу).
    caption_slots_shape(
        depth, cx, bands, mt, mb, caps_top, caps_bot, cuts, forced, solid, h_box,
    )
}
