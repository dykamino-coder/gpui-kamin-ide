//! Охватчики колонок `column-span` и контейнер колонок.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::grid_stack;
use crate::layout::fragment::push::avoid_only_monolith;
use crate::layout::fragment::table_bands::table_box;
use crate::render::{
    block_level_in_flow, is_blank, out_of_flow, real_inline, split_block_in_inline,
};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

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

/// Виден ли фрагмент предка: непустое содержимое или кромка коробки на
/// своей стороне разреза. Пустой фрагмент с кромкой обязателен —
/// css-multicol-1 §column-span, `Overview.bs:1540-1541`: «If the fragment
/// before the spanner is empty, nothing special happens; the top
/// margin/border/padding is above the spanning element, as an empty
/// fragment».
fn spanner_frag_visible(c: &Element, kids: &[Node], first: bool, last: bool) -> bool {
    if kids.iter().any(|n| !is_blank(n)) {
        return true;
    }
    let b = &c.style.border_width;
    (first
        && (spanner_edge(&c.style.margin.top)
            || spanner_edge(&c.style.padding.top)
            || spanner_edge(&b.top)))
        || (last
            && (spanner_edge(&c.style.margin.bottom)
                || spanner_edge(&c.style.padding.bottom)
                || spanner_edge(&b.bottom)))
}

/// Фрагмент предка спаннера. css-break-3 §4.3 (вид `slice`, умолчание
/// `box-decoration-break`): верхние поле/рамка/отбивка — только у первого
/// фрагмента, нижние — только у последнего.
///
/// `keep_size` — отдать фрагменту ЗАДАННУЮ блочную высоту предка. Она
/// принадлежит коробке ЦЕЛИКОМ и расходуется фрагментами по очереди (Blink
/// `fragmentation_utils.cc`: остаток блочного размера считается от уже
/// уложенных фрагментов). Разложить остаток по фрагментам на уровне дерева
/// нечем — геометрия колонок здесь ещё не известна, — поэтому высота
/// ставится ТОЛЬКО когда предок на деле не разошёлся: видимый фрагмент
/// один. Разошёлся на несколько — каждый меряется по содержимому, а не
/// повторяет `height` предка целиком (иначе `height: 200px` удвоилась бы:
/// `non-adjacent-spanners-001`).
fn spanner_fragment(
    c: &Element,
    kids: Vec<Node>,
    first: bool,
    last: bool,
    keep_size: bool,
    ix: usize,
) -> Element {
    let mut f = c.clone();
    f.children = kids;
    if !first {
        f.style.margin.top = None;
        f.style.padding.top = None;
        f.style.border_width.top = None;
        // Устойчивый номер узла у продолжения свой: по нему GPUI хранит
        // состояние (анимация, буферы линеек промежутков), и два фрагмента
        // с одним номером слились бы в один.
        f.node_id = c.node_id ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
    if !last {
        f.style.margin.bottom = None;
        f.style.padding.bottom = None;
        f.style.border_width.bottom = None;
    }
    if !keep_size {
        f.style.height = None;
        f.style.min_height = None;
    }
    f
}

/// Доли заданной блочной высоты предка по его фрагментам вокруг спаннеров.
/// Blink (`fragmentation_utils.cc`, `FinishFragmentation`) расходует
/// `block-size` коробки фрагментами ПО ОЧЕРЕДИ: непоследний фрагмент берёт
/// своё содержимое, но не больше остатка, последний — весь остаток
/// (`multicol-span-all-children-height-004a`: 450 = 200 + 200 + 50, `-005`:
/// 250 = 100 + 100 + 50; `non-adjacent-spanners-000`, `parallel-flow-after-
/// spanner-002`: до спаннера содержимого нет — вся высота ПОСЛЕ него).
/// `None` — правило не берётся (высота не в точках, фрагмент один, виден
/// один и не после пустых, мера не вышла), и действует прежний `keep_size`.
/// В векторе: `Some(v)` — высота фрагмента, `None` — своя, по содержимому.
fn spanner_height_share(c: &Element, bodies: &[Vec<Node>]) -> Option<Vec<Option<f32>>> {
    let Some(Len::Px(total)) = c.style.height else {
        return None;
    };
    let n = bodies.len();
    if n < 2 {
        return None;
    }
    let empty = |b: &Vec<Node>| b.iter().all(is_blank);
    let pre_empty = bodies[..n - 1].iter().all(&empty);
    let seen = bodies
        .iter()
        .enumerate()
        .filter(|(i, b)| spanner_frag_visible(c, b, *i == 0, *i + 1 == n))
        .count();
    if !pre_empty && seen <= 1 {
        return None;
    }
    let px = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let edge = px(&c.style.padding.top)? + px(&c.style.borders().top)?;
    let mut left = total;
    let mut out = Vec::with_capacity(n);
    for (i, b) in bodies.iter().enumerate() {
        if i + 1 == n {
            out.push(Some(left.max(0.0)));
            break;
        }
        if empty(b) {
            out.push(None);
            continue;
        }
        let f = spanner_fragment(c, b.clone(), i == 0, false, false, i);
        let h = shape_full(&f, 4, ShapeCx::COLUMNS)?.0;
        let own = (h - if i == 0 { edge } else { 0.0 }).max(0.0);
        let take = own.min(left);
        out.push((own > left + 0.01).then_some(take));
        left -= take;
    }
    Some(out)
}

/// Разложить содержимое предка на чередование «кусок обычного потока» —
/// «спаннер», рекурсивно вынимая спаннеров из проходимых потомков. Список
/// всегда начинается и кончается куском потока (возможно пустым).
fn spanner_parts(kids: &[Node]) -> Vec<SpanPart> {
    // Спаннер под строчным предком: сперва разорвать строчные на анонимные
    // блоки (§9.2.1.1) — тогда спаннер виден на этом уровне и режет поток,
    // как прямой (`splits_for_spanner`).
    let split;
    let kids: &[Node] = if kids
        .iter()
        .any(|n| matches!(n, Node::Element(k) if splits_for_spanner(k) && has_deep_spanner(k)))
    {
        split = split_block_in_inline(kids);
        &split
    } else {
        kids
    };
    let mut out: Vec<SpanPart> = Vec::new();
    let mut body: Vec<Node> = Vec::new();
    for n in kids {
        match n {
            Node::Element(c) if spanner_box(c) => {
                out.push(SpanPart::Body(std::mem::take(&mut body)));
                out.push(SpanPart::Span(n.clone()));
            }
            Node::Element(c) if passes_spanner(c) && has_deep_spanner(c) => {
                let inner = spanner_parts(&c.children);
                let bodies: Vec<Vec<Node>> = inner
                    .iter()
                    .filter_map(|p| match p {
                        SpanPart::Body(b) => Some(b.clone()),
                        SpanPart::Span(_) => None,
                    })
                    .collect();
                let n_b = bodies.len();
                let shown = bodies
                    .iter()
                    .enumerate()
                    .filter(|(i, b)| spanner_frag_visible(c, b, *i == 0, *i + 1 == n_b))
                    .count();
                // Доли заданной высоты предка (`spanner_height_share`); без
                // них — прежнее правило `keep` ниже.
                let share = spanner_height_share(c, &bodies);
                let mut bi = 0usize;
                for p in inner {
                    match p {
                        SpanPart::Body(b) => {
                            let first = bi == 0;
                            let last = bi + 1 == n_b;
                            // Заданную высоту берёт только НЕ разошедшийся
                            // предок (см. `spanner_fragment`); пустой
                            // фрагмент без кромки и без такой высоты
                            // показывать нечем — он просто исчезает.
                            let own = share.as_ref().and_then(|s| s.get(bi).copied().flatten());
                            let keep = share.is_none() && first && shown <= 1;
                            if spanner_frag_visible(c, &b, first, last)
                                || own.is_some_and(|v| v > 0.0)
                                || (keep && c.style.height.is_some())
                                || (keep && c.style.min_height.is_some())
                            {
                                let mut f = spanner_fragment(c, b, first, last, keep, bi);
                                if let Some(v) = own {
                                    f.style.height = Some(Len::Px(v));
                                    f.style.min_height = None;
                                }
                                body.push(Node::Element(f));
                            }
                            bi += 1;
                        }
                        SpanPart::Span(s) => {
                            // Спаннер потомка поднимается на НАШ уровень и
                            // режет уже наш поток: предки разрезаются вместе
                            // с ним (§column-span). Прозрачность предка при
                            // этом остаётся на спаннере: «Although the
                            // spanner is taken out-of-flow, this does not
                            // affect the painting order of the spanning
                            // element» (`Overview.bs:1471-1472`), а группа
                            // прозрачности — часть отрисовки
                            // (`spanner-in-opacity`). Трансформ и фильтр
                            // сюда не попадают вовсе: они барьер
                            // (`passes_spanner`).
                            let s = match (c.style.opacity, &s) {
                                (Some(o), Node::Element(sp)) if o < 1.0 => {
                                    let mut sp = sp.clone();
                                    sp.style.opacity = Some(sp.style.opacity.unwrap_or(1.0) * o);
                                    Node::Element(sp)
                                }
                                _ => s,
                            };
                            // Родитель спаннера в ИСХОДНОМ дереве. После подъёма
                            // спаннеры разных предков стоят рядом, но между ними
                            // лежит граница предка (его пустой фрагмент в ряду
                            // колонок, §column-span `Overview.bs:1540`), и поля их
                            // НЕ схлопываются (`multicol-span-all-margin-nested-
                            // 001`: «the bottom margin of the first h4 element
                            // should not collapse with the top margin of
                            // div#child»). Метку ставит ближайший предок; глубже
                            // уже помеченные не трогаются. Читает её сегментный
                            // путь в `element()`.
                            let s = match s {
                                Node::Element(mut sp) if sp.attr("kamin-span-parent").is_none() => {
                                    sp.attrs.push((
                                        "kamin-span-parent".to_string(),
                                        c.node_id.to_string(),
                                    ));
                                    Node::Element(sp)
                                }
                                other => other,
                            };
                            out.push(SpanPart::Body(std::mem::take(&mut body)));
                            out.push(SpanPart::Span(s));
                        }
                    }
                }
            }
            other => body.push(other.clone()),
        }
    }
    out.push(SpanPart::Body(body));
    out
}

/// Поднять спаннеров-потомков к прямым детям многоколоночника, разрезав их
/// предков (css-multicol-1 §column-span, `Overview.bs:1497-1499`). `None` —
/// поднимать нечего, дерево не трогаем.
///
/// Blink держит для этого отдельный путь `ColumnSpannerPath`
/// (`column_spanner_path.h`: «A path from the multicol container and down to
/// a column spanner, each container represented as a step on the path») и
/// ведёт раскладку предков по нему: `BlockLayoutAlgorithm` на шаге пути
/// обрывает свой фрагмент перед спаннером
/// (`block_layout_algorithm.cc:1052-1058`), а `ColumnLayoutAlgorithm`
/// достаёт сам спаннер (`GetSpannerFromPath`,
/// `column_layout_algorithm.cc:224`) и кладёт его между линиями колонок. У
/// нас раскладка колонок принимает спаннера ТОЛЬКО прямым ребёнком
/// (`render.rs` `is_span`, `StackChild::span`), поэтому тот же разрез
/// делается в дереве до неё.
pub(crate) fn hoist_spanners(kids: &[Node]) -> Option<Vec<Node>> {
    if !kids.iter().any(|n| {
        matches!(n, Node::Element(c)
            if (passes_spanner(c) || splits_for_spanner(c)) && has_deep_spanner(c))
    }) {
        return None;
    }
    let mut out: Vec<Node> = Vec::with_capacity(kids.len() + 2);
    for p in spanner_parts(kids) {
        match p {
            SpanPart::Body(b) => out.extend(b),
            SpanPart::Span(s) => out.push(s),
        }
    }
    Some(out)
}

/// Многоколоночный контейнер: `column-*` применяются только к блочным
/// контейнерам (css-multicol-1 §2), сетка и гибкий контейнер ими не
/// становятся (`grid-multicol-001`,
/// `column-property-should-not-apply-on-grid-container-001`).
/// Решает ли ширину этой коробки её СОДЕРЖИМОЕ.
///
/// Ключевые слова `min-content`/`max-content`/`fit-content` требуют
/// внутреннего размера прямо (css-sizing-3 §4.1); у плавающей, абсолютной и
/// строчной коробки то же самое зовётся shrink-to-fit (CSS 2.1 §10.3.5) — тот
/// же перечень, что у предиката `shrink_to_fit` в `apply.rs:940`. Элемент
/// гибкого контейнера и сетки тоже меряется содержимым: его основа —
/// `max-content` (css-flexbox-1 §9.2 п.3.A).
pub(super) fn intrinsic_inline_size(c: &Computed, parent: &Computed) -> bool {
    if matches!(
        c.width,
        Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
    ) {
        return true;
    }
    if !matches!(c.width, None | Some(Len::Auto)) {
        return false;
    }
    c.float.unwrap_or(0) != 0
        || matches!(
            c.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        || matches!(
            c.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
        )
        || matches!(
            parent.display,
            Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::GridLanes)
        )
}

pub(crate) fn multicol_container(c: &Computed) -> bool {
    // Заданный `column-height` тоже делает коробку многоколоночной
    // (css-multicol-2 §multi-column-model: «whose column-width, column-count,
    // or column-height property is not auto»; `column-height-012`).
    (c.column_count.is_some() || c.column_width.is_some() || c.column_height.is_some())
        && !matches!(
            c.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::GridLanes)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        )
}
