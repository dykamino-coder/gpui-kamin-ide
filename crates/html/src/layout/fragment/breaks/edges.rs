//! Разрывы и запреты на кромках детей: break-before/after и avoid между соседями.

use crate::dom::{Element, Node};
use crate::layout::fragment::flex_lines::class_a_box;
use crate::layout::fragment::probe::forced_opaque;
use crate::layout::table::is_cell;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::Display;

/// Запрет разрыва на краю коробки: свой `break-before: avoid*` /
/// `break-after: avoid*` либо такой же у ПЕРВОГО/ПОСЛЕДНЕГО поточного
/// блочного ребёнка, рекурсивно (css-break-4 §break-propagation — та же
/// строка спеки, что и у `edge_break` ниже). Зеркало `edge_break`, только
/// для запрещающих значений.
///
/// Ветки «бок о бок» (ряд `flex` без переноса, ячейки одного ряда, один ряд
/// сетки) сюда НЕ перенесены НАМЕРЕННО. Они переносят значение с ЛЮБОГО
/// ребёнка, а не только с крайнего, и на `avoid` это сразу ломает зелёные:
/// в `grid-item-fragmentation-032` запрет стоит на ВТОРОМ элементе сетки
/// рядом с `break-before: column` на третьем, в
/// `single-line-column-flex-fragmentation-016` — на ТРЕТЬЕМ элементе
/// колоночного флекса. Через крайнего ребёнка ни тот, ни другой не проходит,
/// и обе пары остаются нетронутыми. Перенос «бок о бок» для запретов —
/// отдельный шаг с отдельным замером.
pub(crate) fn edge_avoid(e: &Element, last: bool) -> bool {
    let own = if last {
        e.style.break_after_avoid
    } else {
        e.style.break_before_avoid
    };
    if own {
        return true;
    }
    let mut live = e.children.iter().filter(|n| !is_blank(n)).filter(
        |n| !matches!(n, Node::Element(k) if matches!(k.style.display, Some(Display::None))),
    );
    let edge = if last { live.next_back() } else { live.next() };
    // Ряд `flex` без переноса — одна строка: `break-before/after` ЛЮБОГО
    // элемента переносится на строку (css-flexbox-1 §12: «In a row flex
    // container, the break-before and break-after values on flex items are
    // propagated to the flex line»), а единственная строка — первая и
    // последняя, значит значение уходит на контейнер. Сетка и колонка сюда
    // не попадают: там «бок о бок» неверен (см. выше).
    let row_line = matches!(
        e.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) && e.style.webkit_box != Some(true)
        && matches!(
            e.style.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && e.style.flex_wrap != Some(true);
    if row_line {
        return e
            .children
            .iter()
            .filter(|n| !is_blank(n))
            .any(|n| matches!(n, Node::Element(k) if class_a_box(k) && edge_avoid(k, last)));
    }
    matches!(edge, Some(Node::Element(k)) if class_a_box(k) && edge_avoid(k, last))
}

/// Принудительный разрыв на краю коробки: свой `break-before`/`break-after`
/// либо такой же у ПЕРВОГО/ПОСЛЕДНЕГО поточного блочного ребёнка, рекурсивно
/// (css-break-4 §break-propagation: «a 'break-before' value on a first
/// in-flow child box is propagated to its container. Likewise a
/// 'break-after' value on a last in-flow child box»; Blink
/// `BoxFragmentBuilder::SetInitialBreakBefore`). Текст или строчный на краю
/// — анонимная коробка без разрыва, пропагация останавливается.
pub(crate) fn edge_break(e: &Element, last: bool) -> bool {
    // Абсолютная коробка вне потока: `break-*` применяется к блочным коробкам
    // ПОТОКА (css-break-3 §3.1 «Applies to: block-level boxes …»), а внутри
    // своего потока абсолют фрагментируется отдельно (Blink: OOF ложится во
    // фрагментаинер после потока, `out_of_flow_layout_part.cc`) — разрыв его
    // и его потомков потоку родителя не передаётся
    // (`out-of-flow-in-multicolumn-005`).
    if matches!(
        e.style.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        return false;
    }
    let own = if last {
        e.style.break_after_force
    } else {
        e.style.break_before_force
    };
    if own {
        return true;
    }
    // Свой разрыв монолита стоит (выше), разрыв его ПОТОМКА наружу не идёт
    // (`forced_opaque`). `monolithic-content-with-forced-break-001`: `break-after:
    // column` у ребёнка `contain: size` уводил соседа в новую колонку, баланс
    // шёл 50 | 150 вместо 100 | 100. Зелёные `-002/-003` держатся на разрыве
    // САМОЙ монолитной коробки — он до этой строки.
    if forced_opaque(e) {
        return false;
    }
    let mut live = e.children.iter().filter(|n| !is_blank(n)).filter(
        |n| !matches!(n, Node::Element(k) if matches!(k.style.display, Some(Display::None))),
    );
    // Ряд flex БЕЗ переноса: элементы стоят бок о бок и НАЧИНАЮТСЯ с верха
    // ряда — в блочном направлении «первый» и «последний» это каждый из них
    // (css-flexbox-1 §pagination: принудительный разрыв элемента поднимается
    // на контейнер; Blink `flex_layout_algorithm.cc` — разрыв элемента рвёт
    // весь ряд). Мерка ряда — та же, что в `shape_full` и `pushed_box_at`.
    // Проба `target/probe-9g/p-single-line-row-flex-fragmentation-018.html`
    // (`break-after: column` со ВТОРОГО элемента, поднят на `#flex`) = 0.00.
    // ВНИМАНИЕ: `edge_break` зовут и СТРАНИЦЫ (`render_paged`, строки
    // 956-963) — контроль обязан включать `single-line-row-flex-
    // fragmentation-046-print`.
    let row_nowrap = (matches!(
        e.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || e.style.webkit_box == Some(true))
        && matches!(
            e.style.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && e.style.flex_wrap != Some(true)
        && e.style.webkit_box_vertical != Some(true);
    // Ячейки одного ряда стоят бок о бок ровно как элементы ряда `flex` без
    // переноса: в блочном направлении «первой» и «последней» служит КАЖДАЯ
    // (css-break-4 §break-propagation вместе с css-tables-3 §fragmentation;
    // Blink рвёт весь ряд, если разрыв стоит в любой его ячейке). Пока мерка
    // молчала, `break-after` ПЕРВОЙ из двух ячеек терялся, и пары спасал
    // только отказ меры на таблице с голыми ячейками — это записано в
    // комментарии `shape_full` перед `table_box(c)`: «сквозной путь по тегу
    // хранит перенос принудительного разрыва ячейки на таблицу
    // (`break-after-table-cell`, `-child`: 0.00 → 2.08 без гейта)». Х2 этот
    // отказ снимает, значит перенос обязан жить здесь.
    let cells_abreast = e
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .any(|n| matches!(n, Node::Element(k) if is_cell(k)));
    // Элементы ОДНОГО ряда сетки стоят бок о бок ровно как ячейки: в
    // блочном направлении «первым» и «последним» служит КАЖДЫЙ из них
    // (css-grid-2 §Fragmenting Grid Layout: «The 'break-before' property on
    // the first row and the 'break-after' property on the last row are
    // propagated to the grid container»; Blink `grid_layout_algorithm.cc`
    // берёт `InitialBreakBefore`/`FinalBreakAfter` у КАЖДОГО элемента ряда).
    // Берётся только однозначный случай — сетка, у которой ряд ровно ОДИН:
    // колонок больше одной, явных дорожек рядов нет, областей нет, у детей
    // нет `grid-row`/`grid-area`, а поточных детей не больше, чем колонок.
    // Одноколоночная сетка сюда не попадает НАМЕРЕННО: там у каждого
    // ребёнка свой ряд, и первым/последним остаётся ровно первый/последний,
    // как и было (`grid-item-fragmentation-042` — зелёная, гейт её не
    // пускает: `grid-template-columns: 25px`, одна колонка).
    let grid_one_row = matches!(
        e.style.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) && e.style.grid_rows.is_none()
        && e.style.grid_areas.is_none()
        && !matches!(
            e.style.grid_auto_flow,
            Some(crate::style::computed::AutoFlow::Col)
                | Some(crate::style::computed::AutoFlow::ColDense)
        )
        && !e.children.iter().any(|n| {
            matches!(n, Node::Element(k)
            if k.style.grid_row.is_some() || k.style.grid_area_name.is_some())
        })
        && e.style
            .grid_cols
            .map(|n| n as usize)
            .or_else(|| e.style.grid_tracks.as_ref().map(|t| t.len()))
            .is_some_and(|cols| {
                cols > 1
                    && e.children
                        .iter()
                        .filter(|n| !is_blank(n))
                        .filter(|n| {
                            matches!(n, Node::Element(k)
                            if !out_of_flow(&k.style)
                                && !matches!(k.style.display, Some(Display::None)))
                        })
                        .count()
                        <= cols
            });
    if row_nowrap || cells_abreast || grid_one_row {
        return live
            .any(|n| matches!(n, Node::Element(k) if class_a_box(k) && edge_break(k, last)));
    }
    let edge = if last { live.next_back() } else { live.next() };
    matches!(edge, Some(Node::Element(k)) if class_a_box(k) && edge_break(k, last))
}
