//! Сборка StackChild ребёнка стопки колонок: вложенный ряд, монолитность, число копий.

use super::stack_child_of;
use crate::dom::{Element, Node};
use crate::layout::fragment::line_shape::nested_rows_box;
use crate::layout::fragment::probe::{plain_block_tree, size_monolith};
use crate::layout::page::paged::visible_overflow;
use crate::layout::positioned::predicates::carries_abspos;
use crate::render::is_blank;
use crate::style::computed::Display;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_stack_child(
    ix: usize,
    mt: f32,
    mb: f32,
    e: &Element,
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
    dec: Option<(f32, f32)>,
    copy: &Element,
    cuts: Vec<(f32, f32)>,
    forced: Vec<f32>,
    solid: Vec<(f32, f32)>,
    h: f32,
    over: f32,
    rel: (f32, f32),
    nest_row: Option<f32>,
    whole: bool,
    build: impl Fn(bool, usize) -> gpui::AnyElement,
) -> crate::layout::fragment::types::StackChild {
    let (scrolls, monolith) = copy_monolith(copy, &cuts, nest_row);
    // Высоту меряет раскладка копии (`StackChild::measure`):
    // точек разреза мера не дала, и строчный набор без них
    // не монолит — режется краем колонки. Монолит — только
    // по собственным причинам коробки (css-break-3 §4.1).
    let (measure, monolith) =
        measured_monolith(measured_kids, copy, nest_row, whole, scrolls, monolith);
    // Пока строятся копии — «внутри стопки»: вложенный
    // многоколоночник со спаннером остаётся на
    // сегментном пути (см. `unified` выше).
    let _nested = crate::layout::fragment::types::StackScope::enter();
    let span = copy.style.column_span == Some(true) && !copy.inline;
    // Переполняющие колонки (css-multicol-1 §8.2: «A multicol
    // container can have more columns than it has room for due
    // to: a declaration that constrains the column height … In
    // this case, additional column boxes are created in the
    // inline direction») — ТОЛЬКО ребёнку, который несёт
    // абсолютного потомка: его содержащий блок сплошной, и
    // абсолют режется по колонкам сам (css-position-3
    // §abspos-breaking), а копий у ребёнка было ровно
    // `column-count` — хвост уходил «за кадр» (`flow.rs`
    // `fill_at`, `copy + 1 >= limit`;
    // `out-of-flow-in-multicolumn-007`: CB 300 при колонке 100,
    // копий 2 из 3). ★ Прежний патч без гейта «несёт абсолют»
    // (scout-fragoof-2026-09d §7) замерен +7/−14: все потери —
    // дети БЕЗ абсолютов (вложенные многоколоночники, флекс,
    // `multicol-fill-balance-*`), у которых мера `shape_full`
    // не совпадает с рисунком. Гейт `plain_block_tree` — тот же,
    // что у параллельного потока выше. Прочим детям — прежнее
    // число копий, и стопка без такого ребёнка байт-в-байт
    // прежняя (`ColumnStack::new` берёт наибольшее число копий).
    let kid_copies = kid_copy_count(e, rows, copies, fixed, copy, h, over, measure, span);
    stack_child_of(
        ix, mt, mb, kid_par, col_vert, rows, fixed, dec, copy, cuts, forced, solid, h, over, rel,
        build, measure, monolith, span, kid_copies,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn nest_row_of(
    ix: usize,
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    fixed_nest: Option<f32>,
    balanced_frag: Option<f32>,
    nest_at: &[Option<f32>],
    nested_auto: &std::cell::RefCell<Vec<u64>>,
    copy: &Element,
) -> Option<f32> {
    fixed_nest.filter(|hh| {
        *hh > 0.0
            && (rows.is_none() || balanced_frag.is_some())
            && !col_vert
            && kid_par.get(ix).is_none_or(|p| p.group == 0)
            && nested_rows_box(copy)
            && match nest_at.get(ix).copied().flatten() {
                // С верха колонки — и заданная высота, и
                // `auto` с мерой рядами.
                Some(y0) if y0 < 0.01 => {
                    matches!(copy.style.height, Some(Len::Px(_)))
                        || nested_auto.borrow().contains(&copy.node_id)
                }
                // Ниже верха — только заданная высота: мера
                // коробки от рядов не зависит.
                Some(_) => matches!(copy.style.height, Some(Len::Px(_))),
                None => false,
            }
    })
}

pub(super) fn measured_monolith(
    measured_kids: &std::cell::RefCell<Vec<(u64, f32)>>,
    copy: &Element,
    nest_row: Option<f32>,
    whole: bool,
    scrolls: impl Fn(Option<crate::style::computed::Overflow>) -> bool,
    monolith: bool,
) -> (Option<f32>, bool) {
    let measure = measured_kids
        .borrow()
        .iter()
        .find(|(id, _)| *id == copy.node_id)
        .map(|(_, w)| *w);
    let monolith = if whole {
        true
    } else if measure.is_some() {
        nest_row.is_none()
            && (size_monolith(copy)
                || copy.style.break_inside_avoid
                || scrolls(copy.style.overflow_x)
                || scrolls(copy.style.overflow_y))
    } else {
        monolith
    };
    (measure, monolith)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn kid_copy_count(
    e: &Element,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    copy: &Element,
    h: f32,
    over: f32,
    measure: Option<f32>,
    span: bool,
) -> usize {
    match fixed {
        Some(per)
            if rows.is_none()
                && !span
                && per > 0.0
                && visible_overflow(&e.style)
                && visible_overflow(&copy.style)
                && plain_block_tree(copy, 4)
                && carries_abspos(copy, 4) =>
        {
            ((h.max(over) / per).ceil() as usize + 1)
                .min(16)
                .max(copies)
        }
        // Высота неизвестна до раскладки: копий — на
        // переполняющие колонки (css-multicol-1 §8.2).
        Some(per) if measure.is_some() && rows.is_none() && !span && per > 0.0 => {
            copies.max(8).min(16)
        }
        _ => copies,
    }
}

pub(super) fn copy_monolith(
    copy: &Element,
    cuts: &[(f32, f32)],
    nest_row: Option<f32>,
) -> (
    impl Fn(Option<crate::style::computed::Overflow>) -> bool + use<>,
    bool,
) {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(k)
            if !k.inline || k.style.display == Some(Display::Block))
    };
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): `contain: size` как
    // монолит (Blink `IsMonolithic`) — срез фрагментации
    // 469 -> 467 (+1/−3): `single-line-column-flex-
    // fragmentation-051/063` режутся у Blink иначе (рост
    // элемента от фрагментации, корень R5 скаута).
    // Рост лёг `705fd58`; `contain: size` — `size_monolith`,
    // тот же предикат, что у `solid_box` в мере и пробе
    // `grow_pushed` (`scout-break-2026-09e.md`).
    let monolith = nest_row.is_none()
        && (size_monolith(copy)
        || copy.style.break_inside_avoid
        || scrolls(copy.style.overflow_x)
        || scrolls(copy.style.overflow_y)
        || matches!(
            copy.tag.as_str(),
            "img"
                | "svg"
                | "canvas"
                | "video"
                | "embed"
                | "object"
                | "iframe"
        )
        // Таблица и ячейка — не монолиты
        // (css-break-4 §4.1).
        || matches!(
            copy.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
        )
        // Сплошной СТРОЧНЫЙ набор тоже монолит:
        // резать его можно лишь между строками, а
        // строк укладка колонок не видит, и разрез
        // приходился бы посреди строки.
        // ПУСТАЯ коробка с высотой режется по своей
        // высоте (css-break-4 §4.2; корень A3).
        || (copy.children.iter().any(|n| !is_blank(n))
            && !copy.children.iter().any(block_kid)
            // Строки измерены (`line_run_shape`) — режется
            // между строк, не монолит.
            && cuts.is_empty()));
    (scrolls, monolith)
}
