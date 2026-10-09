//! Флекс-строки при фрагментации.
// owner: A

use crate::style::cascade::inherit::inherit;
use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::probe::size_monolith;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::page::paged::visible_overflow;
use crate::layout::positioned::predicates::carries_abspos;
use crate::render::{is_blank, out_of_flow};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

/// Многострочный КОЛОНОЧНЫЙ flex-контейнер — ребёнок стопки колонок —
/// раскрывается в свои элементы, разложенные по строкам (`flow::Par`): строки
/// такого контейнера — параллельные потоки (Blink `flex_layout_algorithm.cc`
/// :2108-2112 — `FlexColumnBreakInfo` на каждую строку; :2504-2515 — разрыв
/// элемента переходит к следующей СТРОКЕ, а не обрывает контейнер; :2536-2560
/// — рост элемента от фрагментации двигает только его строку,
/// `item_offset_adjustment`). Прежде контейнер мерился стопкой ВСЕХ
/// элементов подряд (`shape_full`, ветка `flex_items`), и строки ложились одна
/// под другой (`multi-line-column-flex-fragmentation-*`: «красное видно»).
///
/// Строки — по css-flexbox-1 §9.3 (шаг 5, «collect consecutive items one by one
/// until the first time that the next collected item would not fit into the
/// flex container's inner main size»): главный размер — внешняя высота меры
/// элемента, между элементами `row-gap`. Поперечный — наибольшая внешняя
/// ширина в строке; `align-content: normal` = `stretch` раздаёт свободное место
/// строкам поровну (§9.4 шаг 15 / css-align-3 §5.4), элемент `width: auto` при
/// `align-items: normal` тянется на строку (§9.4 шаг 11). Коробка контейнера —
/// первая «строка» группы без детей: рисует его фон под элементами и занимает
/// его высоту и тогда, когда строки короче.
///
/// Гейт узкий — ровно то, что выражается без раскладки: колонка с переносом
/// (не `reverse`), высота в точках, ни полей, ни рамок, ни отбивок у
/// контейнера, `justify-content`/`align-content`/`align-items` по умолчанию,
/// дети — блочные элементы в потоке без `flex-grow`, `flex-basis`,
/// `align-self`, боковых полей и отбивок, ширина в точках либо пустой
/// `auto`. Иначе контейнер идёт прежним путём.
pub(crate) fn split_flex_lines(
    kids: Vec<(Element, Shape)>,
    col_w: Option<f32>,
    merged: &Computed,
) -> (Vec<(Element, Shape)>, Vec<crate::layout::fragment::types::Par>, Vec<Option<Computed>>, Vec<usize>) {
    let mut out: Vec<(Element, Shape)> = Vec::with_capacity(kids.len());
    let mut par: Vec<crate::layout::fragment::types::Par> = Vec::with_capacity(kids.len());
    let mut parent: Vec<Option<Computed>> = Vec::with_capacity(kids.len());
    let mut group = 0u32;
    let mut starts: Vec<usize> = Vec::with_capacity(kids.len() + 1);
    for (c, s) in kids {
        starts.push(out.len());
        match flex_lines_of(&c, col_w) {
            Some(lines) => {
                group += 1;
                let pm = inherit(merged, &c.style);
                let h = match c.style.height {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                // Коробка контейнера — ПЕРВОЙ «строкой» группы: рисуется под
                // элементами (фон контейнера, `multi-line-column-flex-
                // fragmentation-033`) и занимает свою высоту, даже когда строки
                // короче. `break-before` первых элементов ВСЕХ строк — на неё,
                // `break-after` последних — на последний элемент группы (Blink
                // `flex_layout_algorithm.cc:1907-1918`: колонки строк —
                // «ряд», значения сливаются и уходят контейнеру; «avoid» +
                // принудительный = принудительный, `JoinFragmentainerBreakValues`).
                let heads: Vec<&Element> = lines.iter().filter_map(|l| l.1.first().map(|x| &x.0)).collect();
                let tails: Vec<&Element> = lines.iter().filter_map(|l| l.1.last().map(|x| &x.0)).collect();
                let (bf, ba) = (
                    heads.iter().any(|e| edge_break(e, false)),
                    heads.iter().any(|e| edge_avoid(e, false)),
                );
                let (af, aa) = (
                    tails.iter().any(|e| edge_break(e, true)),
                    tails.iter().any(|e| edge_avoid(e, true)),
                );
                let mut boxc = c.clone();
                boxc.node_id = c.node_id ^ 0x0F1E_5BAC_E000_0001;
                boxc.children = Vec::new();
                boxc.style.display = Some(Display::Block);
                boxc.style.flex_dir = None;
                boxc.style.flex_wrap = None;
                boxc.style.gap = None;
                boxc.style.break_before_force = bf;
                boxc.style.break_before_avoid = ba && !bf;
                boxc.style.break_after_force = false;
                boxc.style.break_after_avoid = false;
                out.push((boxc, (h, 0.0, 0.0, Vec::new(), Vec::new(), Vec::new())));
                par.push(crate::layout::fragment::types::Par {
                    group,
                    group_start: true,
                    line_start: true,
                    group_end: false,
                    dx: 0.0,
                    avoid_only: false,
                    float: false,
                    clears: false,
                });
                parent.push(None);
                let n_lines = lines.len();
                for (li, (dx, items)) in lines.into_iter().enumerate() {
                    let m = items.len();
                    for (ii, (mut e, sh)) in items.into_iter().enumerate() {
                        if li + 1 == n_lines && ii + 1 == m {
                            e.style.break_after_force |= af;
                            e.style.break_after_avoid |= aa && !af;
                        }
                        // `break-inside: avoid` без настоящего монолита
                        // (`flow::Par::avoid_only`).
                        let avoid_only = e.style.break_inside_avoid
                            && !size_monolith(&e)
                            && visible_overflow(&e.style);
                        out.push((e, sh));
                        par.push(crate::layout::fragment::types::Par {
                            group,
                            group_start: false,
                            line_start: ii == 0,
                            group_end: false,
                            dx,
                            avoid_only,
                            float: false,
                    clears: false,
                        });
                        parent.push(Some(pm.clone()));
                    }
                }
                if let Some(p) = par.last_mut() {
                    p.group_end = true;
                }
            }
            None => match flex_row_lines_of(&c, col_w) {
                // Многострочный РЯД: строки идут одна за другой, а элементы
                // строки — параллельные потоки своей группы (Blink
                // `flex_layout_algorithm.cc:2167-2213`: элемент ряда — свой
                // поток, конец ряда — самый дальний конец его элементов).
                Some(lines) => {
                    let pm = inherit(merged, &c.style);
                    for items in lines {
                        group += 1;
                        let m = items.len();
                        for (ii, (dx, e, sh)) in items.into_iter().enumerate() {
                            let avoid_only = e.style.break_inside_avoid
                                && !size_monolith(&e)
                                && visible_overflow(&e.style);
                            out.push((e, sh));
                            par.push(crate::layout::fragment::types::Par {
                                group,
                                group_start: ii == 0,
                                line_start: true,
                                group_end: ii + 1 == m,
                                dx,
                                avoid_only,
                                float: false,
                    clears: false,
                            });
                            parent.push(Some(pm.clone()));
                        }
                    }
                }
                None => {
                    out.push((c, s));
                    par.push(crate::layout::fragment::types::Par::default());
                    parent.push(None);
                }
            },
        }
    }
    starts.push(out.len());
    (out, par, parent, starts)
}

/// Строки контейнера для `split_flex_lines`: `(сдвиг строки по x, элементы с
/// мерой)`. `None` — контейнер вне гейта.
#[allow(clippy::type_complexity)]
pub(crate) fn flex_lines_of(c: &Element, col_w: Option<f32>) -> Option<Vec<(f32, Vec<(Element, Shape)>)>> {
    use crate::style::computed::FlexDir;
    if flex_gap_rules(&c.style) {
        return None;
    }
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let s = &c.style;
    let b = s.borders();
    if c.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || s.flex_dir != Some(FlexDir::Col)
        || s.flex_wrap != Some(true)
        || s.flex_wrap_reverse == Some(true)
        || s.flex_balance == Some(true)
        || s.vertical == Some(true)
        || s.justify_content.is_some()
        || s.align_content.is_some()
        || s.align_items.is_some()
        // `position: relative` без сдвигов ничего не двигает, а содержащим
        // блоком ему служить некому (внепоточных детей гейт не пускает).
        || !(s.position.is_none()
            || (s.position == Some(crate::style::computed::Position::Relative)
                && [&s.inset.top, &s.inset.right, &s.inset.bottom, &s.inset.left]
                    .into_iter()
                    .all(|l| matches!(l, None | Some(Len::Auto)))))
        || s.transform.is_some()
        || s.min_height.is_some()
        || s.max_height.is_some()
        || !visible_overflow(s)
        || ![
            &s.margin.top,
            &s.margin.bottom,
            &s.margin.left,
            &s.margin.right,
            &s.padding.top,
            &s.padding.bottom,
            &s.padding.left,
            &s.padding.right,
            &b.top,
            &b.bottom,
            &b.left,
            &b.right,
        ]
        .into_iter()
        .all(zero)
    {
        return None;
    }
    let Some(Len::Px(main)) = s.height else {
        return None;
    };
    let cross = match s.width {
        Some(Len::Px(w)) => w,
        None | Some(Len::Auto) => col_w?,
        _ => return None,
    };
    let (row_gap, col_gap) = match s.gap {
        None => (0.0, 0.0),
        Some((r, g)) => {
            let px = |l: &Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(v.max(0.0)),
                _ => None,
            };
            (px(&r)?, px(&g)?)
        }
    };
    let mut items: Vec<&Element> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        let ks = &k.style;
        if k.inline
            || out_of_flow(ks)
            || !matches!(ks.position, None | Some(crate::style::computed::Position::Relative))
            || ks.float.unwrap_or(0) != 0
            || ks.flex_grow.is_some_and(|g| g > 0.0)
            || ks.flex_basis.is_some()
            || ks.align_self.is_some()
            || ks.align_self_normal
            || !zero(&ks.margin.left)
            || !zero(&ks.margin.right)
        {
            return None;
        }
        items.push(k);
    }
    if items.is_empty() {
        return None;
    }
    // Визуальный порядок (`order`, стабильно), как в `blocks()`.
    items.sort_by_key(|k| k.style.order.unwrap_or(0));
    // Мера и внешний поперечный размер элемента; `None` у ширины — `auto`.
    let mut measured: Vec<(Element, Shape, Option<f32>)> = Vec::with_capacity(items.len());
    for k in items {
        let sh = shape_full(k, 4, ShapeCx::COLUMNS)?;
        let kb = k.style.borders();
        let side = |l: &Option<Len>| match l {
            None => Some(0.0),
            Some(Len::Px(v)) => Some(*v),
            _ => None,
        };
        let w = match k.style.width {
            Some(Len::Px(w)) => {
                let extra = if k.style.border_box == Some(true) {
                    0.0
                } else {
                    side(&k.style.padding.left)?
                        + side(&k.style.padding.right)?
                        + side(&kb.left)?
                        + side(&kb.right)?
                };
                Some(w + extra)
            }
            None | Some(Len::Auto)
                if zero(&k.style.padding.left)
                    && zero(&k.style.padding.right)
                    && zero(&kb.left)
                    && zero(&kb.right)
                    && k.children.iter().all(is_blank) =>
            {
                None
            }
            _ => return None,
        };
        measured.push((k.clone(), sh, w));
    }
    // Строки: §9.3 шаг 5.
    let mut lines: Vec<Vec<(Element, Shape, Option<f32>)>> = Vec::new();
    let mut used = 0.0f32;
    for (k, sh, w) in measured {
        let outer = sh.0 + sh.1 + sh.2;
        match lines.last_mut() {
            Some(line) if used + row_gap + outer <= main + 0.01 => {
                used += row_gap + outer;
                line.push((k, sh, w));
            }
            _ => {
                used = outer;
                lines.push(vec![(k, sh, w)]);
            }
        }
    }
    // Одна строка — однострочный по сути контейнер: прежний путь его знает.
    if lines.len() < 2 {
        return None;
    }
    let n = lines.len() as f32;
    let crosses: Vec<f32> = lines
        .iter()
        .map(|l| l.iter().filter_map(|x| x.2).fold(0.0f32, f32::max))
        .collect();
    let free = cross - crosses.iter().sum::<f32>() - col_gap * (n - 1.0);
    let extra = if free > 0.0 { free / n } else { 0.0 };
    let mut out = Vec::with_capacity(lines.len());
    let mut dx = 0.0f32;
    for (line, lc) in lines.into_iter().zip(crosses) {
        let lc = lc + extra;
        let mut items = Vec::with_capacity(line.len());
        for (i, (mut k, mut sh, w)) in line.into_iter().enumerate() {
            // `auto` тянется на строку (§9.4 шаг 11, `align-self: stretch`).
            if w.is_none() {
                k.style.width = Some(Len::Px(lc));
            }
            // Зазор между элементами строки — к полю следующего: на разрыве
            // он пропадает вместе с полем (Blink
            // `UpdateOffsetAdjustmentForSuppressedRowGap`, :2486-2500).
            if i > 0 {
                sh.1 += row_gap;
            }
            items.push((k, sh));
        }
        out.push((dx, items));
        dx += lc + col_gap;
    }
    Some(out)
}

/// Линейки промежутков у flex-контейнера (css-gaps-1 `column-rule`/`row-rule`).
/// Строки, раскрытые в параллельные потоки (`split_flex_lines`), — отдельные
/// копии без контейнера, и художник линеек (`GapRulePainter`) их не видит:
/// линейки пропадали целиком (`flex-gap-decorations-fragmentation-025/028/029/
/// 030`, v225 0.04…0.28 → v226 0.82…2.07). Такой контейнер идёт прежним путём —
/// одной копией со своими линейками.
pub(crate) fn flex_gap_rules(s: &Computed) -> bool {
    s.column_rule_visible == Some(true)
        || s.row_rule_visible == Some(true)
        || s.column_rule_styles.as_ref().is_some_and(|l| l.any(|v| *v))
        || s.row_rule_styles.as_ref().is_some_and(|l| l.any(|v| *v))
}

/// Строки многострочного РЯДА flex для `split_flex_lines`: по строке —
/// `(сдвиг по x, элемент, мера)` каждого элемента. Строки — по css-flexbox-1
/// §9.3 шаг 5 (главная ось — ширина), поперечный размер строки — наибольшая
/// внешняя высота её элементов; элемент `height: auto` при `align-items:
/// normal` тянется на строку (§9.4 шаг 11) — полом `min-height`, чтобы рост
/// от фрагментации (`grow_pushed`) коробку не обрезал (Blink: «expansion past
/// the block-end of each row», `flex_layout_algorithm.cc:2560-2575`).
/// `break-before` любого элемента строки — разрыв перед строкой, `break-after`
/// — после неё (`:1898-1906`): на первого и последнего элемента строки. Гейт —
/// как у колонки, плюс высота контейнера `auto` и хотя бы одна строка из
/// нескольких элементов: ряд «элемент на строку» прежний путь уже знает.
#[allow(clippy::type_complexity)]
pub(crate) fn flex_row_lines_of(c: &Element, col_w: Option<f32>) -> Option<Vec<Vec<(f32, Element, Shape)>>> {
    use crate::style::computed::FlexDir;
    if flex_gap_rules(&c.style) {
        return None;
    }
    let zero = |l: &Option<Len>| match l {
        None => true,
        Some(Len::Px(v)) => v.abs() < 0.01,
        _ => false,
    };
    let s = &c.style;
    let b = s.borders();
    if c.inline
        || s.display != Some(Display::Flex)
        || s.webkit_box == Some(true)
        || !matches!(s.flex_dir, None | Some(FlexDir::Row))
        || s.flex_wrap != Some(true)
        || s.flex_wrap_reverse == Some(true)
        || s.flex_balance == Some(true)
        || s.vertical == Some(true)
        || s.rtl == Some(true)
        || s.justify_content.is_some()
        || s.align_content.is_some()
        || s.align_items.is_some()
        || !matches!(s.height, None | Some(Len::Auto))
        || s.min_height.is_some()
        || s.max_height.is_some()
        || s.transform.is_some()
        || s.background.is_some()
        || s.bg_image.is_some()
        || !(s.position.is_none()
            || (s.position == Some(crate::style::computed::Position::Relative)
                && [&s.inset.top, &s.inset.right, &s.inset.bottom, &s.inset.left]
                    .into_iter()
                    .all(|l| matches!(l, None | Some(Len::Auto)))))
        || !visible_overflow(s)
        || ![
            &s.margin.top,
            &s.margin.bottom,
            &s.margin.left,
            &s.margin.right,
            &s.padding.top,
            &s.padding.bottom,
            &s.padding.left,
            &s.padding.right,
            &b.top,
            &b.bottom,
            &b.left,
            &b.right,
        ]
        .into_iter()
        .all(zero)
    {
        return None;
    }
    let main = match s.width {
        Some(Len::Px(w)) => w,
        None | Some(Len::Auto) => col_w?,
        _ => return None,
    };
    let (row_gap, col_gap) = match s.gap {
        None => (0.0, 0.0),
        Some((r, g)) => {
            let px = |l: &Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(v.max(0.0)),
                _ => None,
            };
            (px(&r)?, px(&g)?)
        }
    };
    let side = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let mut items: Vec<&Element> = Vec::new();
    for n in c.children.iter().filter(|n| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        let ks = &k.style;
        if k.inline
            || out_of_flow(ks)
            || !matches!(ks.position, None | Some(crate::style::computed::Position::Relative))
            || ks.float.unwrap_or(0) != 0
            || ks.flex_grow.is_some_and(|g| g > 0.0)
            || ks.align_self.is_some()
            || ks.align_self_normal
            || ks.min_height.is_some()
            || !zero(&ks.margin.left)
            || !zero(&ks.margin.right)
            || row_item_width(ks, main).is_none()
        {
            return None;
        }
        items.push(k);
    }
    if items.is_empty() {
        return None;
    }
    items.sort_by_key(|k| k.style.order.unwrap_or(0));
    // Строки по внешней ширине элементов (главная ось).
    let mut lines: Vec<Vec<(f32, Element, Shape)>> = Vec::new();
    let mut used = 0.0f32;
    // Строка из одного элемента и размер в процентах/`flex-basis` — шире
    // прежнего гейта: такой ряд прежде шёл целым контейнером, и его мера
    // (`shape_full` контейнера) уже знала рост строки от разрыва внутри
    // элемента, растяжение соседей на выросшую строку, статическое место
    // абсолютного потомка и вложенный параллельный поток. Раскрытые элементы
    // этого не выражают (`grow_pushed` растит лишь сам элемент): замерено
    // −3 (`multi-line-row-flex-fragmentation-053/060/062`). Такие элементы —
    // прежним путём.
    let widened = items.iter().any(|k| !matches!(k.style.width, Some(Len::Px(_))) || k.style.flex_basis.is_some());
    let mut single = true;
    let mut risky = false;
    for k in &items {
        risky |= carries_abspos(k, 4) || constrained_inside(k, 4);
    }
    for k in items {
        let kb = k.style.borders();
        // Гипотетический главный размер (css-flexbox-1 §9.2 шаг 3):
        // `flex-basis` в точках/процентах, иначе `width`; проценты — от
        // главного размера контейнера (§9.2 «percentage … against the flex
        // container's inner main size»). Копия элемента несёт его в точках:
        // в стопке он рисуется блоком в колонке.
        let w = row_item_width(&k.style, main)?;
        let mut k = k.clone();
        k.style.width = Some(Len::Px(w));
        k.style.flex_basis = None;
        let k = &k;
        let w = if k.style.border_box == Some(true) {
            w
        } else {
            w + side(&k.style.padding.left)?
                + side(&k.style.padding.right)?
                + side(&kb.left)?
                + side(&kb.right)?
        };
        let sh = shape_full(k, 4, ShapeCx::COLUMNS)?;
        match lines.last_mut() {
            Some(line) if used + col_gap + w <= main + 0.01 => {
                single = false;
                line.push((used + col_gap, k.clone(), sh));
                used += col_gap + w;
            }
            _ => {
                lines.push(vec![(0.0, k.clone(), sh)]);
                used = w;
            }
        }
    }
    if (single || widened) && (risky || lines.iter().flatten().any(|x| !x.2.4.is_empty())) {
        return None;
    }
    for (li, line) in lines.iter_mut().enumerate() {
        let cross = line.iter().map(|x| x.2.0 + x.2.1 + x.2.2).fold(0.0f32, f32::max);
        let (bf, ba) = (
            line.iter().any(|x| edge_break(&x.1, false)),
            line.iter().any(|x| edge_avoid(&x.1, false)),
        );
        let (af, aa) = (
            line.iter().any(|x| edge_break(&x.1, true)),
            line.iter().any(|x| edge_avoid(&x.1, true)),
        );
        let m = line.len();
        for (ii, (_, e, sh)) in line.iter_mut().enumerate() {
            // `height: auto` тянется на строку — полом.
            if matches!(e.style.height, None | Some(Len::Auto)) && sh.0 + sh.1 + sh.2 < cross - 0.01 {
                let eb = e.style.borders();
                let edges = side(&e.style.padding.top).unwrap_or(0.0)
                    + side(&e.style.padding.bottom).unwrap_or(0.0)
                    + side(&eb.top).unwrap_or(0.0)
                    + side(&eb.bottom).unwrap_or(0.0);
                let content = (cross - sh.1 - sh.2 - edges).max(0.0);
                e.style.min_height = Some(Len::Px(content));
                *sh = shape_full(e, 4, ShapeCx::COLUMNS)?;
            }
            // Зазор между строками — к полю элементов следующей строки.
            if li > 0 {
                sh.1 += row_gap;
            }
            if ii == 0 {
                e.style.break_before_force |= bf;
                e.style.break_before_avoid |= ba && !bf;
            }
            if ii + 1 == m {
                e.style.break_after_force |= af;
                e.style.break_after_avoid |= aa && !af;
            }
        }
    }
    Some(lines)
}

/// В поддереве (до `depth`) — коробка с заданной высотой и содержимым: свой
/// параллельный поток (css-break-3 §3), которого раскрытый элемент ряда не
/// выражает (`flex_row_lines_of`).
pub(crate) fn constrained_inside(c: &Element, depth: u8) -> bool {
    depth > 0
        && c.children.iter().any(|n| match n {
            Node::Element(k) => {
                (matches!(k.style.height, Some(Len::Px(_)) | Some(Len::Pct(_)))
                    && k.children.iter().any(|n| !is_blank(n)))
                    || constrained_inside(k, depth - 1)
            }
            _ => false,
        })
}

/// Главный размер элемента многострочного ряда для `flex_row_lines_of`:
/// `flex-basis` (точки/проценты) при `flex-grow: 0`, иначе `width`; проценты —
/// от главного размера контейнера `main`. `None` — размер по содержимому
/// (`auto`/`content`), который гейт не выражает.
pub(crate) fn row_item_width(ks: &Computed, main: f32) -> Option<f32> {
    let px = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => Some(*v),
        Some(Len::Pct(p)) => Some(p * main),
        _ => None,
    };
    if ks.basis_content == Some(true) {
        return None;
    }
    match ks.flex_basis {
        Some(Len::Auto) | None => px(&ks.width),
        _ => px(&ks.flex_basis),
    }
}

/// Коробка, дающая точку разрыва класса A (css-break-4 §possible-breaks):
/// блочная, в потоке, не плавающая. `Element.inline` ставится по ТЕГУ
/// (`dom.rs` `INLINE_TAGS`), поэтому `<img style="display: block; page: b">`
/// блочным тут признаётся по `display` (`page-name-img-004`: иначе картинка
/// шла анонимным блоком с именем корня и рвала страницу).
pub(crate) fn class_a_box(e: &Element) -> bool {
    let blocky = (!e.inline && !inline_display(e))
        || matches!(
            e.style.display,
            Some(Display::Block)
                | Some(Display::Flex)
                | Some(Display::Grid)
                | Some(Display::Table)
                | Some(Display::ListItem)
        );
    blocky
        && !out_of_flow(&e.style)
        && e.style.float.unwrap_or(0) == 0
        && !matches!(
            e.style.display,
            Some(Display::None) | Some(Display::Contents)
        )
}

/// Флекс- и грид-контейнер: его дети — элементы раскладки, не блоки потока.
/// 'page' применяется только к коробкам с точками разрыва класса A
/// (css-page-3 §page-prop «Applies to: boxes that create class A break
/// points»), и имя элемента флекса/грида контейнеру не передаётся и
/// разрыва между элементами не ставит (`page-name-flex-001/002-print`:
/// эталон без разрывов). Внутри элемента — обычный блочный поток
/// (`page-name-flex-004-print`).
pub(crate) fn item_container(e: &Element) -> bool {
    matches!(
        e.style.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
    )
}

/// Строчный уровень по `display` у элемента с блочным тегом: `<div
/// style="display: inline-block">` стоит в строке и точки класса A не даёт
/// (css-display-3 §inner-outer; `page-name-inline-block-003-print`: два
/// таких `div` с разными `page` — одна строка, без разрыва).
pub(crate) fn inline_display(e: &Element) -> bool {
    // `display: inline` у блочного тега хранится как `InlineBlock` с меткой
    // `inline_display`: блоки внутри такого строчного разрывают его
    // (block-in-inline), и их разрывы — точки класса A
    // (`css-break/block-in-inline-015-print`). Его не трогаем.
    e.style.inline_display != Some(true)
        && matches!(
        e.style.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    )
}
