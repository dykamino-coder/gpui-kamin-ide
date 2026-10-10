//! Охватчики колонок `column-span` и контейнер колонок.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::grid_bands::grid_stack;
use crate::layout::fragment::push::avoid_only_monolith;
use crate::layout::fragment::table_bands::table_box;
use crate::render::{block_level_in_flow, out_of_flow, real_inline};
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod fragment;
mod hoist;
pub(crate) use hoist::hoist_spanners;
pub(super) use hoist::intrinsic_inline_size;
pub(crate) use hoist::multicol_container;

/// Есть ли в поддереве (вместе с самой коробкой) элементы РЯДА: гибкий
/// контейнер, сетка, таблица, вложенный многоколоночник. Их элементы
/// фрагментируются каждый своими точками (Blink `flex_layout_algorithm.cc`,
/// `table_row_layout_algorithm.cc`), а мера стопки сливает их монолиты в один
/// диапазон (`shape_full`, ветка `row_nowrap`; `table_shape`, объединение по
/// ячейкам). Переполнить колонку «до конца монолита» такому ребёнку значит увести
/// в переполнение и соседей по ряду, поэтому `Kid::overflow_top` им не дают.
/// Атомарные строчные (`inline-flex`/`inline-grid`) — монолиты, но внутрь них
/// спуск всё равно идёт: гейт осторожный.
pub(crate) fn parallel_items_inside(c: &Element, depth: u8) -> bool {
    // Элементы, идущие СТОПКОЙ по блочной оси, рядом не стоят: колонка flex
    // без переноса (css-flexbox-1 §9.3: одна строка, элементы друг под
    // другом) и сетка-стопка (`grid_stack`: одна колонка, ряд = элемент).
    // Монолит такого элемента переполняет колонку, как в блоке (Blink
    // `FinishFragmentation` растит фрагмент контейнера до конца монолита;
    // `monolithic-overflow-003/004.tentative` во flex и сетке).
    let stacked = (c.style.display == Some(Display::Flex)
        && c.style.webkit_box != Some(true)
        && c.style.vertical != Some(true)
        && matches!(
            c.style.flex_dir,
            Some(crate::style::computed::FlexDir::Col)
                | Some(crate::style::computed::FlexDir::ColReverse)
        )
        && c.style.flex_wrap != Some(true))
        || (c.style.display == Some(Display::Grid) && grid_stack(c));
    // `break-inside: avoid` элемента без настоящего монолита — пожелание
    // (css-break-4 §4.4): с верха колонки такой элемент выше колонки рвётся, а
    // ветка переполнения (`overflow_top`) держала бы его целым
    // (`single-line-column-flex-fragmentation-015`: элемент 250 в колонке 100).
    let stacked = stacked
        && !c.children.iter().any(
            |n| matches!(n, Node::Element(k) if k.style.break_inside_avoid && avoid_only_monolith(k)),
        );
    let own = !stacked
        && matches!(
            c.style.display,
            Some(Display::Flex)
                | Some(Display::Grid)
                | Some(Display::GridLanes)
                | Some(Display::Table)
                | Some(Display::TableRow)
                | Some(Display::TableRowGroup)
                | Some(Display::TableCell)
        )
        || c.style.webkit_box == Some(true)
        || multicol_container(&c.style)
        || matches!(
            c.tag.as_str(),
            "table" | "tr" | "td" | "th" | "thead" | "tbody" | "tfoot"
        );
    own || (depth > 0
        && c.children
            .iter()
            .any(|n| matches!(n, Node::Element(k) if parallel_items_inside(k, depth - 1))))
}

/// Есть ли в поддереве (вместе с самой коробкой) многоколоночник. Свою
/// балансировку он ведёт сам, внешними колонками не фрагментируется, и копия
/// стопки рисует его плоско (`flow.rs` `StackChild::nested_cols`).
pub(super) fn multicol_inside(c: &Element, depth: u8) -> bool {
    multicol_container(&c.style)
        || (depth > 0
            && c.children
                .iter()
                .any(|n| matches!(n, Node::Element(k) if multicol_inside(k, depth - 1))))
}

/// Кусок содержимого предка спаннера: обычный поток или сам спаннер.
/// css-multicol-1 §column-span (`Overview.bs:1353-1355`): спаннер «forces a
/// column break and is taken out of flow to span across all columns of the
/// nearest multicol ancestor», то есть режет содержимое предка на «до»,
/// «спаннер» и «после».
enum SpanPart {
    Body(Vec<Node>),
    Span(Node),
}

/// Коробка со спаннером: `column-span: all` применяется только к
/// внутрипоточным блочным элементам (css-multicol-1 §column-span,
/// «Applies to: in-flow block-level elements»; Blink
/// `LayoutBox::IsSelfValidColumnSpanner`). Блочный УРОВЕНЬ — по `display`,
/// а не по тегу (`Element::inline` теговый, `dom.rs:2892`):
/// `<div style="display: inline-block">` спаннером не бывает
/// (`inline-block-and-column-span-all`: флоаты вставали рядом во всю ширину
/// коробки, 200×50 вместо 100×100), `<span style="display: block">` —
/// бывает; флоат и абсолют — не в потоке. Тот же признак — у `is_span`
/// сегментного пути в `element()`.
pub(crate) fn spanner_box(c: &Element) -> bool {
    c.style.column_span == Some(true) && block_level_in_flow(c)
}

/// Пропускает ли предок спаннера его наружу, к многоколоночнику.
///
/// Спека (css-multicol-1 §column-span, `Overview.bs:1497-1499`): «A spanning
/// element may be lower than the first level of descendants as long as they
/// are part of the same formatting context, and there is nothing between the
/// spanning element and multicol container that establishes a containing
/// block for fixed position descendants».
///
/// Blink проверяет то же в `LayoutBox::DoesAncestryAllowColumnSpanner` →
/// `ShouldPreventColumnSpannerDescendants` (`layout_box.cc:2860-2900`):
/// предок обязан быть блочным контейнером потока (`LayoutBlockFlow`), не
/// монолитом, не порождать своего контекста форматирования
/// (`CreatesNewFormattingContext`) и не быть содержащим блоком для
/// фиксированных потомков (`CanContainFixedPositionObjects`); спаннер
/// внутри спаннера тоже запрещён.
fn passes_spanner(c: &Element) -> bool {
    // «No spanners inside spanners in the same multicol context».
    if spanner_box(c) {
        return false;
    }
    // Строчная коробка — не блочный контейнер потока: спаннер внутри
    // `<span>` живёт в анонимной коробке блок-в-строчном, и вынуть его
    // отсюда, не разобрав саму анонимную коробку, нечем.
    if c.inline {
        return false;
    }
    // Таблица и её внутренние коробки — не блочный контейнер потока, хотя
    // `display` у них пуст: табличный вид решается ТЕГОМ (`table_box`), и
    // проверка `display` ниже их пропускала. Blink
    // `DoesAncestryAllowColumnSpanner`: предок обязан быть `LayoutBlockFlow`
    // без своего контекста форматирования, а таблица, ячейка и подпись им
    // не являются. Без барьера `<table><caption><h3 style="column-span:
    // all">` вынимался наружу, и `multicol-span-all-004` («non-spanner in
    // caption») расходилась с эталоном, где тот же `<h3>` —
    // `column-span: none`.
    if table_box(c)
        || c.style.is_caption == Some(true)
        || matches!(
            c.tag.as_str(),
            "caption" | "colgroup" | "col" | "thead" | "tbody" | "tfoot" | "tr" | "td" | "th"
        )
    {
        return false;
    }
    if !matches!(
        c.style.display,
        None | Some(Display::Block) | Some(Display::ListItem)
    ) {
        return false;
    }
    // Свой многоколоночник: спаннер принадлежит БЛИЖАЙШЕМУ предку-
    // многоколоночнику, а не нашему (§column-span: «the nearest multicol
    // ancestor in the same block formatting context»).
    if multicol_container(&c.style) {
        return false;
    }
    // Внепоточный и плавающий предок — свой контекст форматирования.
    if out_of_flow(&c.style) {
        return false;
    }
    if c.style.flow_root == Some(true) {
        return false;
    }
    if !matches!(
        c.style.overflow_x,
        None | Some(crate::style::computed::Overflow::Visible)
    ) || !matches!(
        c.style.overflow_y,
        None | Some(crate::style::computed::Overflow::Visible)
    ) {
        return false;
    }
    // Содержащий блок для фиксированных потомков (§column-span, пример с
    // `transform: rotate(90deg)`: «The transform establishes a containing
    // block for fixed position descendents, therefore a spanner will not be
    // created»). На этом запрете стоят зелёные `multicol-span-all-010`
    // (`transform`, `filter`, `contain: paint|layout|content|strict`) и
    // `multicol-span-all-017` (`transform: scale(1)`).
    if c.style.transform.is_some()
        || c.style.filter.is_some()
        || c.style.perspective.is_some()
        || c.style.contain_layout == Some(true)
        || c.style.contain_paint == Some(true)
    {
        return false;
    }
    true
}

/// Есть ли в поддереве спаннер, достижимый через проходимых предков.
pub(crate) fn has_deep_spanner(c: &Element) -> bool {
    c.children.iter().any(|n| match n {
        Node::Element(k) if spanner_box(k) => true,
        Node::Element(k) if passes_spanner(k) || splits_for_spanner(k) => has_deep_spanner(k),
        _ => false,
    })
}

/// Строчный предок спаннера. Блок внутри строчного рвёт его на анонимные
/// блоки (CSS 2.1 §9.2.1.1, `split_block_in_inline`), и спаннер оказывается
/// ребёнком АНОНИМНОГО блочного контейнера — а тот проходим (Blink:
/// анонимный блок — тоже `LayoutBlockFlow`, `DoesAncestryAllowColumnSpanner`
/// его пропускает; `multicol-span-float-003`, `parallel-flow-after-spanner-
/// 001`). Атомарные строчные (`inline-block`, кнопка, замещаемые) — барьер:
/// их `real_inline` не пускает.
fn splits_for_spanner(k: &Element) -> bool {
    real_inline(k) && !out_of_flow(&k.style)
}

/// Кромка коробки на стороне разреза: есть ли что показывать ПУСТОМУ
/// фрагменту. `Len::Px(0)` кромкой не считается.
fn spanner_edge(l: &Option<Len>) -> bool {
    !matches!(l, None | Some(Len::Px(0.0)))
}
