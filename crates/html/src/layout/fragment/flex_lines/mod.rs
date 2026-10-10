//! Флекс-строки при фрагментации.
// owner: A

use crate::dom::Element;
use crate::layout::fragment::Shape;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::probe::size_monolith;
use crate::layout::page::paged::visible_overflow;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
mod column;
pub(super) use column::flex_gap_rules;
use column::flex_lines_of;
mod row;
use row::flex_row_lines_of;
mod items;
pub(crate) use items::class_a_box;
pub(crate) use items::inline_display;
pub(crate) use items::item_container;
use items::{constrained_inside, row_item_width};

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
) -> (
    Vec<(Element, Shape)>,
    Vec<crate::layout::fragment::types::Par>,
    Vec<Option<Computed>>,
    Vec<usize>,
) {
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
                let heads: Vec<&Element> = lines
                    .iter()
                    .filter_map(|l| l.1.first().map(|x| &x.0))
                    .collect();
                let tails: Vec<&Element> = lines
                    .iter()
                    .filter_map(|l| l.1.last().map(|x| &x.0))
                    .collect();
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
