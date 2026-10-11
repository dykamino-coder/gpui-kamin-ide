//! Div фрагмента копии: стиль, ширина колонки, линейки промежутков.

use crate::dom::{Element, Node};
use crate::layout::fragment::grid_bands::grid_rows_px;
use crate::render::styled_div_with;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{IntoElement, Styled, px};

#[allow(clippy::too_many_arguments)]
pub(super) fn frag_box_div(
    col_vert: bool,
    col_rl: bool,
    copy: &Element,
    h: f32,
    src: &Element,
    src_inner: &Computed,
    frag_gap_rules: Option<crate::paint::gap_rules::GapRuleSpec>,
    frag_gap_key: Option<u64>,
    kids: Vec<Node>,
    body: &mut Vec<gpui::AnyElement>,
) -> gpui::Div {
    let mut d = styled_div_with(src, src_inner);
    // Ось блочного потока ВНУТРИ копии — горизонтальная
    // (css-writing-modes-4 §3.1), как у вертикального
    // блока в `element()`: гибкий ряд, у `vertical-rl`
    // обратный. Гибкому и сеточному ось ставит `apply`.
    if col_vert && matches!(src.style.display, None | Some(Display::Block)) {
        d = d.flex();
        d = if col_rl {
            d.flex_row_reverse()
        } else {
            d.flex_row()
        };
    }
    // Голый `styled_div_with` — БЛОК taffy (`apply.rs`
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
                    .aspect_ratio
                    .is_some_and(|r| r.is_finite() && r > 0.0)
                && matches!(k.style.width, None | Some(Len::Auto))
                && matches!(k.style.height, Some(Len::Px(_))))
    };
    if src.style.display.is_none() && src_inner.vertical != Some(true) && kids.iter().any(ratio_kid)
    {
        d = d.flex().flex_col();
    }
    // Для ЛЮБОЙ flex/grid-копии, не только с линейками:
    // эталоны css-gaps (`…-fragmentation-008-ref`) кладут
    // ту же сетку без правил, и с гейтом «только с
    // линейками» тест рисовал сетку, а эталон — нет
    // (v93: 008 3.75, 009 5.18, 010 4.50).
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v94): то же для
    // flex-копий. css-break 2874: +26/−15, и девять потерь
    // — 99.00 (`multi-line-row-flex-fragmentation-084…090`,
    // `multi-line-column-flex-fragmentation-056/057`:
    // страница разъезжается), ещё 065–071 на 1.5–13.
    // Сетка даёт +17 в css-break без потерь.
    // Flex-копия — во всю ширину колонки, как и
    // сетка: flex-корень с `width: auto` taffy
    // кладёт шириной СОДЕРЖИМОГО (`flexbox.rs`
    // `determine_container_main_size`, ветвь
    // `Definite` → `longest_line_length`), и
    // элемент `width: 100%` выходил нулевым, а
    // `width: 100px` в колонке 50 не сжимался.
    // Замер одного этого (v94): +9/−15, все
    // потери — `row-gap`, их закрывает мера
    // гибкой стопки (`flex_items` в `shape_full`).
    if matches!(copy.style.width, None | Some(Len::Auto))
        && matches!(
            copy.style.display,
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
            copy.style.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && grid_rows_px(&copy.style).is_some()
    {
        d = d.min_h(px(h));
    }
    if let (Some(key), Some(spec)) = (frag_gap_key, frag_gap_rules) {
        // Копия кладётся `layout_as_root(Definite(col_w), …)`
        // (`flow.rs` `ColumnStack::prepaint`), а taffy у
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
            crate::paint::gap_rules::painter::GapRulePainter::new(
                crate::paint::gap_rules::gap_items_for(key),
                spec,
            )
            .into_any_element(),
        );
    }
    d
}
