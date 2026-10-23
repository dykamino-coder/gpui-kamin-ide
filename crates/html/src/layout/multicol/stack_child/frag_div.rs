//! Div фрагмента копии: стиль, ширина колонки, линейки промежутков.

use crate::dom::{Element, Node};
use crate::hooks::IntoHooked;
use crate::layout::fragment::grid_bands::grid_rows_px;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
use gpui::{Styled, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn frag_box_div(
    col_vert: bool,
    col_rl: bool,
    copy: &Element,
    h: f32,
    src: &Element,
    src_inner: &BoxStyle,
    frag_gap_rules: Option<crate::paint::gap_rules::GapRuleSpec>,
    frag_gap_items: Option<crate::paint::gap_rules::GapItems>,
    kids: Vec<Node>,
    body: &mut Vec<gpui::AnyElement>,
    cx: &crate::layout::cx::LayoutCx,
) -> gpui::Div {
    let mut d = cx.build.styled_box(src, src_inner, cx);
    // Ось блочного потока ВНУТРИ копии — горизонтальная
    // (css-writing-modes-4 §3.1), как у вертикального
    // блока в `element()`: гибкий ряд, у `vertical-rl`
    // обратный. Гибкому и сеточному ось ставит `apply`.
    if col_vert && matches!(src.style.used().display, None | Some(Display::Block)) {
        d = d.flex();
        d = if col_rl {
            d.flex_row_reverse()
        } else {
            d.flex_row()
        };
    }
    // Голый `styled_div_with` — БЛОК taffy (`paint/apply/`
    // `apply_layout`: блоку вызова нет, gpui `Display::Block`
    // → taffy Block), а блок общего пути — гибкая колонка
    // (`d.flex().flex_col()` при пустом `display`, та же
    // оболочка у спаннера выше). На колонку опирается
    // `blocks()`: коробке с `aspect-ratio`, auto-шириной и
    // высотой в точках он ставит `Align::Start` (css-sizing-4
    // §5.1 «calculated the same as for a replaced element
    // with a natural aspect ratio»; Blink `length_utils.cc:
    // 535-562` → `FitContent`), а блочный алгоритм taffy
    // `align-self` не читает и тянет её во всю ширину
    // родителя. Прежде вылет прятала маска шириной в
    // колонку; после multicol-rest P7 (css-multicol-1 §8.1:
    // «visibly overflows and is not clipped to the column
    // box») он виден (`block-aspect-ratio-052`: зелёный 345
    // вместо 25, четыре фрагмента — 420×100). Гейт узкий —
    // только копия с таким ребёнком; колонка для ЛЮБОЙ
    // копии блока — отдельным замером.
    let ratio_kid = |n: &Node| {
        matches!(n, Node::Element(k)
            if !k.inline
                && k.style
                    .used().aspect_ratio
                    .is_some_and(|r| r.is_finite() && r > 0.0)
                && matches!(k.style.used().width, None | Some(Len::Auto))
                && matches!(k.style.used().height, Some(Len::Px(_))))
    };
    if src.style.used().display.is_none()
        && !src_inner.wm().vertical()
        && kids.iter().any(ratio_kid)
    {
        d = d.flex().flex_col();
    }
    // Для ЛЮБОЙ flex/grid-копии, не только с линейками:
    // эталоны css-gaps (`…-fragmentation-008-ref`) кладут
    // ту же сетку без правил, и с гейтом «только с
    // линейками» тест рисовал сетку, а эталон — нет.
    // Flex-копия — во всю ширину колонки, как и
    // сетка: flex-корень с `width: auto` taffy
    // кладёт шириной СОДЕРЖИМОГО (`flexbox.rs`
    // `determine_container_main_size`, ветвь
    // `Definite` → `longest_line_length`), и
    // элемент `width: 100%` выходил нулевым, а
    // `width: 100px` в колонке 50 не сжимался.
    // Одна лишь полная ширина flex-копии ломает
    // `row-gap` и разваливает страницы
    // `multi-line-*-flex-fragmentation-*`; её
    // закрывает мера гибкой стопки (`flex_items`
    // в `shape_full`) — держать их вместе.
    if matches!(copy.style.used().width, None | Some(Len::Auto))
        && matches!(
            copy.style.used().display,
            Some(Display::Grid) | Some(Display::InlineGrid) | Some(Display::Flex)
        )
    {
        d = d.w_full();
    }
    // Пустая сетка: taffy раскладывает бездетный
    // узел ЛИСТОМ (vendor/taffy/src/tree/
    // taffy_tree.rs: `(_, false) =>
    // compute_leaf_layout`), явные дорожки ему
    // не видны, и копия выходила нулевой при
    // мере 200 (`grid-container-fragmentation-
    // 002`: `grid-template-rows: 200px`). Дорожка
    // существует без элементов (css-grid-1
    // §7.1) — высота копии та же, что в мере.
    if kids.is_empty()
        && matches!(
            copy.style.used().display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && grid_rows_px(&copy.style.used()).is_some()
    {
        d = d.min_h(px(h));
    }
    if let (Some(items), Some(spec)) = (frag_gap_items, frag_gap_rules) {
        // Копия кладётся `layout_as_root(Definite(col_w), …)`
        // (`ColumnStack::prepaint`), а taffy у
        // flex/grid-КОРНЯ с `width: auto` берёт размер
        // содержимого, не доступное место
        // (`vendor/taffy/src/compute/flexbox.rs`
        // `determine_container_main_size`, ветвь
        // `Definite` → `longest_line_length`): дорожки
        // `1fr` выходили нулевыми, и вся сетка была
        // невидима (`grid-gap-decorations-fragmentation-
        // 008/010/016`: только серый фон). Блок
        // растягивается сам; flex/grid получают 100% —
        // корень разрешает долю против `available_space`
        // (`taffy/src/compute/mod.rs` `compute_root_layout`).
        // Под детьми копии — как в `element()`
        // (css-gaps-1: «just above the border»).
        body.insert(
            0,
            crate::hooks::PaintOnly(crate::paint::gap_rules::painter::GapRulePainter::new(
                items,
                spec,
                cx.paint.clone(),
            ))
            .hooked(),
        );
    }
    d
}
