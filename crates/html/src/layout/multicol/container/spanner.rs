//! Сегменты многоколоночника между охватывающими элементами (column-span: all).

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::multicol::column_flow::column_flow;
use crate::layout::options::RenderOpts;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::Display;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

#[allow(clippy::too_many_arguments)]
pub(crate) fn multicol_spanner_segments(
    mut d: gpui::Div,
    e: &Element,
    merged: crate::style::computed::BoxStyle,
    opts: &RenderOpts,
    col_w_px: Option<f32>,
    want: Option<usize>,
    lone_span: bool,
    rest_h: Option<f32>,
    mut span_prev: Option<Option<u64>>,
    mut seg_open: bool,
    is_span: impl Fn(&Node) -> bool,
) -> AnyElement {
    let vcx = opts.cx.vcx();
    // `<fieldset>`: отрисованная легенда стоит ВНЕ колонок —
    // многоколоночность получает анонимная коробка содержимого
    // fieldset (HTML §15.3.13 «The fieldset and legend
    // elements», пересказ; Blink `LayoutFieldset` +
    // `FieldsetContentBox`). Прежде легенда падала в первый ряд и
    // занимала колонку (`multicol-span-all-fieldset-001…003`:
    // эталон — `fieldset > legend + div.inner` с колонками).
    // Отрисованная — первая `legend` в потоке.
    let mut kids = e.children.clone();
    if e.tag == "fieldset"
        && let Some(i) = kids.iter().position(
            |n| matches!(n, Node::Element(c) if c.tag == "legend" && !out_of_flow(&c.style.computed())),
        )
    {
        let legend = kids.remove(i);
        d = d.children(opts.cx.build.blocks(&[legend], &merged, opts));
    }
    for chunk in kids.split_inclusive(&is_span) {
        let (body, span) = match chunk.split_last() {
            Some((last, head)) if is_span(last) => (head, Some(last)),
            _ => (chunk, None),
        };
        if body.iter().any(|n| !is_blank(n)) {
            // Ряд колонок между спаннерами — новый контекст
            // форматирования: поля спаннеров сквозь него не
            // схлопываются (§column-span: «margins on elements
            // inside a column box will not collapse with the margin
            // of a spanner»). Сам ряд для taffy — лист или
            // не-блок, насквозь его поле не проходит.
            span_prev = None;
            seg_open = true;
            let mut seg = e.clone();
            seg.children = body.to_vec();
            seg.style.used_mut().column_span = None;
            // Одна колонка со спаннером (`lone_span`) — ряд рисуется
            // обычным блоком: `column_flow` спускается в
            // единственного блочного ребёнка и рисует только его
            // текст, теряя коробку (фон, рамку, высоту фрагмента
            // `container` в `multicol-span-all-children-height-005/
            // 008`). Клон `sub` с одной колонкой спаннеров не несёт,
            // и гейт `lone_span` его снова не пускает.
            let flow = if lone_span {
                None
            } else {
                column_flow(&seg, &merged, opts, want, col_w_px)
            };
            if let Some(el) = flow {
                d = d.child(el);
            } else {
                // Блочный сегмент: рекурсия в общий рендер —
                // он сам выберет укладку колонок; коробка
                // (фон/рамки/поля) остаётся на хосте.
                let mut sub = seg.clone();
                sub.style.used_mut().background = None;
                // Всё, что многоколоночник рисует и сдвигает КОРОБКОЙ,
                // уже стоит на хосте `d` (`styled_div_with(e, ..)`);
                // клон ряда повторял это на каждом ряду: контур и
                // тень вокруг каждого ряда, двойная прозрачность,
                // фильтр и трансформ, двойной сдвиг `relative`, а у
                // `position: absolute` ряды выпадали из потока и
                // ложились друг на друга (`multicol-span-all-
                // fieldset-002/003`, `-button-002/003`). Обрезку
                // переполнения делает хост: по css-multicol-1
                // §overflow режет коробка многоколоночника, а не ряд.
                // Ряд остаётся содержащим блоком (`relative`), как
                // прежде.
                sub.style.used_mut().gradient = None;
                sub.style.used_mut().bg_image = None;
                sub.style.used_mut().border_image = None;
                sub.style.used_mut().shadows = Vec::new();
                sub.style.used_mut().inset_shadows = Vec::new();
                sub.style.used_mut().outline = None;
                sub.style.used_mut().opacity = None;
                sub.style.used_mut().transform = None;
                sub.style.used_mut().filter = None;
                sub.style.used_mut().mask_image = None;
                sub.style.used_mut().backdrop_blur = None;
                if matches!(
                    sub.style.computed().position,
                    Some(crate::style::computed::Position::Absolute)
                        | Some(crate::style::computed::Position::Fixed)
                ) {
                    sub.style.used_mut().position =
                        Some(crate::style::computed::Position::Relative);
                }
                sub.style.used_mut().inset = Default::default();
                sub.style.used_mut().z_index = None;
                sub.style.used_mut().overflow_x = None;
                sub.style.used_mut().overflow_y = None;
                sub.style.used_mut().margin = Default::default();
                sub.style.used_mut().padding = Default::default();
                sub.style.used_mut().border_width = Default::default();
                sub.style.used_mut().width = None;
                sub.style.used_mut().height = None;
                // Хвост после спаннера при `column-fill: auto` —
                // остаток коробки (`rest_h`): укладка заполняет
                // колонки подряд, а не балансирует. Только
                // измеримым блокам: неизмеримый ряд уходит в
                // запасную сетку, где заданная высота растянула бы
                // дорожки `auto`.
                if span.is_none()
                    && body.iter().all(|n| {
                        is_blank(n)
                            || matches!(n, Node::Element(k)
                                if !k.inline
                                    && shape_full(k, 4, ShapeCx::COLUMNS, vcx).is_some())
                    })
                    && let Some(rest) = rest_h
                {
                    sub.style.used_mut().height = Some(Len::Px(rest));
                }
                d = d.child(div().children(opts.cx.build.blocks(
                    &[Node::Element(sub)],
                    &merged,
                    opts,
                )));
            }
        }
        if let Some(Node::Element(sp)) = span {
            // Хост сегментов — блок taffy (`Display::Block`,
            // `vendor/gpui/src/style.rs:862`), и поля соседних детей
            // он схлопывает сам (`vendor/taffy/src/compute/
            // block.rs:186-210`): соседние спаннеры ОДНОГО предка —
            // верно («the margins of two adjacent spanners will
            // collapse with each other»; `multicol-span-all-margin-
            // 003`). Запрещённое схлопывание гасит нулевая гибкая
            // сторожка — её taffy насквозь не проходит
            // (`has_styles_preventing_being_collapsed_through`:
            // `!style.is_block()`): (1) перед ПЕРВЫМ куском-спаннером
            // — многоколоночник сам контекст форматирования, и
            // верхнее поле спаннера сквозь его верх не уходит;
            // (2) между спаннерами РАЗНЫХ исходных предков (метка
            // `Marks::span_parent`, `spanner_parts`).
            let key = sp.marks.span_parent;
            if !seg_open || span_prev.is_some_and(|k| k != key) {
                d = d.child(div().flex());
            }
            seg_open = true;
            span_prev = Some(key);
            let inner = box_style(&merged, &sp.style, vcx);
            // Спаннер — независимый контекст форматирования
            // (§column-span). Голый `styled_div_with` — блок taffy,
            // и тот схлопывал поле первого/последнего ребёнка
            // сквозь край спаннера (`vendor/taffy/src/compute/
            // block.rs:186`): в `multicol-span-all-margin-nested-
            // firstchild-001` `<span style="margin: 2em 0">` уводил
            // `<h6>` вниз, и чёрного фона не было видно вовсе.
            // Оболочка — гибкая колонка, ровно как у блока общего
            // пути (`d.flex().flex_col()` при пустом `display`), где
            // поля детей сводит сам `blocks()` — а он с предикатом
            // `own_context_style` поле наружу больше не отдаёт.
            let shell = opts.cx.build.styled_box(sp, &inner, &opts.cx);
            let shell =
                if matches!(inner.display, None | Some(Display::Block)) && !inner.wm().vertical() {
                    shell.flex().flex_col()
                } else {
                    shell
                };
            d = d.child(shell.children(opts.cx.build.blocks(&sp.children, &inner, opts)));
        }
    }
    // Нижняя сторожка: нижнее поле последнего спаннера остаётся
    // внутри многоколоночника — он независимый контекст
    // форматирования (CSS 2.1 §8.3.1).
    d = d.child(div().flex());
    return d.into_any_element();
}
