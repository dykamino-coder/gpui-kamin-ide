//! Форма содержимого по детям: флаги разрывов детей и сборка формы из их вырезов.

use super::{finish_shape_cuts, kid_shapes, shaped_height};
use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::edge_avoid;
use crate::layout::fragment::flex_lines::{class_a_box, item_container};
use crate::layout::fragment::shape_kids::stack_kids;
use crate::layout::page::names::page_names;
use crate::render::out_of_flow;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(super) fn shape_from_kids(
    c: &Element,
    depth: u8,
    px_or: impl Fn(&Option<Len>, bool) -> Option<f32>,
    mt: f32,
    mb: f32,
    unclamp: bool,
    cx: ShapeCx,
    top: f32,
    bot: f32,
    kids: Vec<&Node>,
    flex_items: bool,
    flex_col: bool,
    flex_gap: f32,
    oof_kid: Vec<bool>,
    abs_top: Vec<bool>,
    avoid_kid: Vec<(bool, bool)>,
    page_kid: Vec<Option<(String, String)>>,
    page_prev: Option<String>,
    blk_avoid: Vec<(bool, bool)>,
    line_inner: Vec<bool>,
    row_nowrap: bool,
    grid_rows_stack: bool,
    row_gap: f32,
    no_descent: bool,
) -> Option<(f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>)> {
    let inner = kid_shapes(c, depth, &px_or, cx, &kids, flex_col, no_descent);
    let mut cuts: Vec<(f32, f32)> = Vec::new();
    let mut forced: Vec<f32> = Vec::new();
    let mut solid: Vec<(f32, f32)> = Vec::new();
    // Рамка и отбивка самой коробки — без разрывов (Blink:
    // «Avoid breaking inside block-start border»).
    if top > 0.0 {
        solid.push((0.0, top));
    }
    // Стек вложенных: конец, поле первого, схлопнувшееся
    // сквозь верх без отбивки, поле последнего.
    let mut stacked: Option<(f32, f32, f32)> = None;
    // Самый нижний край внепоточных потомков, отсчитанный
    // от верха ЭТОЙ коробки. В поток не входит, высоту
    // соседей не двигает — нужен только фрагментации.
    let mut oof_reach = 0.0f32;
    if let Some(kids) = inner.filter(|k| !k.is_empty()) {
        (stacked, oof_reach) = stack_kids(
            kids,
            row_nowrap,
            top,
            &mut cuts,
            &mut forced,
            &mut solid,
            flex_items,
            oof_kid,
            cx,
            abs_top,
            grid_rows_stack,
            row_gap,
            flex_gap,
            avoid_kid,
            line_inner,
            blk_avoid,
            page_kid,
            oof_reach,
            page_prev,
        );
    }
    // Заданная высота — в точках или (для страниц) в единицах окна.
    let (h, mt, mb) = match shaped_height(
        c,
        depth,
        px_or,
        mt,
        mb,
        unclamp,
        cx,
        top,
        bot,
        kids,
        &mut cuts,
        &mut forced,
        &mut solid,
        stacked,
    ) {
        Ok(value) => value,
        Err(value) => return value,
    };
    // Содержащий блок обязан дотянуться до низа своих
    // внепоточных потомков — только тогда фрагментация
    // родит под них колонки, а балансировка их посчитает
    // (css-position-3 §abspos-breaking; Blink
    // `column_layout_algorithm.cc:1092-1131` прогоняет
    // `OutOfFlowLayoutPart` внутри цикла балансировки
    // именно ради этого). Если коробка содержащим блоком
    // НЕ является, дотяг принадлежит кому-то выше и здесь
    // не учитывается — он всплывёт там.
    finish_shape_cuts(
        c, unclamp, cx, top, bot, cuts, forced, solid, stacked, oof_reach, h, mt, mb,
    )
}

pub(super) fn kid_break_flags(
    c: &Element,
    cx: ShapeCx,
    kids: &Vec<&Node>,
    flex_items: bool,
) -> (
    Vec<bool>,
    Vec<bool>,
    bool,
    Vec<(bool, bool)>,
    Vec<Option<(String, String)>>,
    Option<String>,
    Vec<(bool, bool)>,
) {
    let oof_kid: Vec<bool> = kids
        .iter()
        .map(|n| matches!(n, Node::Element(k) if out_of_flow(&k.style)))
        .collect();
    // Абсолют с заданным `top` стоит от верха содержащего блока, а не на
    // статическом месте (CSS 2.1 §10.6.4): его дотяг у страниц отсчитывается
    // от верха коробки. Прежде — от курсора потока, и `top: 0` после блока
    // 250vh тянул лист на 250vh дальше (`fixedpos-008-print`: девять листов
    // вместо шести).
    let abs_top: Vec<bool> = kids
        .iter()
        .map(|n| {
            matches!(n, Node::Element(k)
                if k.style.position == Some(crate::style::computed::Position::Absolute)
                    && matches!(k.style.inset.top, Some(l) if !matches!(l, Len::Auto)))
        })
        .collect();
    // Запреты `break-before/after: avoid*` элементов гибкой стопки — с
    // переносом с крайних потомков (`edge_avoid`). Сцепки из них (`flex_run`
    // ниже) — только у РЯДА С ПЕРЕНОСОМ: там стопка «строка = элемент» идёт по
    // блочной оси в порядке строк. ★ Потери P6 (свод v219): у колонки сцепка
    // уводила `single-line-column-flex-fragmentation-016/017`,
    // `multi-line-column-flex-fragmentation-027/028` — диапазон сцепки для
    // `fill_at` монолит, и в колонке с заданной высотой ветка `overflow_to`
    // держала в одной колонке элемент в 300px, хотя `avoid` запрещает только
    // ТОЧКУ между элементами (css-break-3 §4.4 правило 1; Blink
    // `fragmentation_utils.cc:266-269` лишь снижает её привлекательность).
    // `wrap-reverse` кладёт строки с другого края, а стопка — в порядке DOM
    // (`multi-line-row-flex-fragmentation-050`).
    let avoid_chains = flex_items
        && c.style.vertical != Some(true)
        && matches!(
            c.style.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap == Some(true)
        && c.style.flex_wrap_reverse != Some(true);
    let avoid_kid: Vec<(bool, bool)> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if avoid_chains => (edge_avoid(k, false), edge_avoid(k, true)),
            _ => (false, false),
        })
        .collect();
    // Те же запреты на границах БЛОЧНЫХ детей (css-break-4 §4.3 правило 1):
    // граница, закрытая `break-after: avoid*` предыдущего или `break-before:
    // avoid*` следующего, точкой разрыва не служит. Разрыв уходит к последней
    // законной точке ВНУТРИ предыдущего ребёнка (Blink `early_break_`,
    // `block_layout_algorithm.cc:1086`; `break-between-avoid-007`: c с
    // `break-before: avoid` после обёрток над a и b — разрыв между a и b).
    // Начальное/конечное имя страницы поточных детей класса A (css-page-3
    // §using-named-pages п. 4): несовпадение конца предыдущего с началом
    // следующего — принудительный разрыв на их границе, и на ЛЮБОЙ глубине
    // (Blink `fragmentation_utils.cc` `CalculateBreakBetweenValue`: имя
    // ребёнка против имени текущего фрагмента контейнера). Только у страниц;
    // `style.page` здесь уже несёт используемое значение (`fill_used_page`).
    let page_kid: Vec<Option<(String, String)>> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if cx.paged && !item_container(c) && class_a_box(k) => {
                Some(page_names(k, ""))
            }
            _ => None,
        })
        .collect();
    let page_prev: Option<String> = None;
    let blk_avoid: Vec<(bool, bool)> = kids
        .iter()
        .map(|n| match n {
            Node::Element(k) if !flex_items && !out_of_flow(&k.style) => {
                (edge_avoid(k, false), edge_avoid(k, true))
            }
            _ => (false, false),
        })
        .collect();
    (
        oof_kid,
        abs_top,
        avoid_chains,
        avoid_kid,
        page_kid,
        page_prev,
        blk_avoid,
    )
}
