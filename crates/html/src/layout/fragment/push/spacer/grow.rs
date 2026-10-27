//! Рост перенесённой коробки, монолиты под avoid и поля абзаца.

use super::super::cell::grow_before;
use super::super::pushed_box_at;
use super::spacer_before;
use crate::document::state::ValueCx;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::{clone_dec, solid_box};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::probe::size_monolith;
use crate::layout::fragment::table_bands::repeat_leads;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::multicol::spanner::parallel_items_inside;
use crate::style::computed::Display;

/// Рост коробки от вытолкнутого монолита (Blink `FinishFragmentation`,
/// `fragmentation_utils.cc:641-656`: у НЕпоследнего фрагмента
/// `final_block_size = space_left`; css-flexbox-1 §fragmentation: «A forced
/// break inside a flex item effectively increases the size of its
/// contents»). Укладка режет ребёнка стопки в НАЧАЛЕ монолитного диапазона
/// `a` (`layout::fragment::plan` `fill_at`: `holds` → `at(a)`): в колонке остаётся
/// `a − from` содержимого при остатке `room`, и коробки, где лежит монолит,
/// обязаны дотянуться до низа, а всё после него — сдвинуться на
/// `grow = room − (a − from)`. Копия одна на все колонки, и сдвиг в ней
/// делает распорка — `margin-top += grow` у самой внешней коробки,
/// начинающейся ровно в `a` (`pushed_box_at`). После неё монолит стоит на
/// краю колонки, мера выросла на `grow`, и срез по краю совпадает с Blink:
/// колонка 1 — `room`, следующая копия — с `from + room`. План зависит
/// только от мер (`ColumnStack::growths`), поэтому распорки ставятся ДО
/// сборки копий, до неподвижной точки (≤ 6 заходов; обычно один). Проба
/// устройства руками: SLC-007/009/010/011 и `table-cell-expansion-001` 0.00.
/// Ребёнка, для которого `measured` истинно, распорками растит раскладка его
/// копии (`push::measured`), и оценка его не правит.
#[allow(clippy::too_many_arguments)]
pub(crate) fn grow_pushed(
    mut kids: Vec<(Element, Shape)>,
    count: usize,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    par: &[crate::layout::fragment::types::Par],
    measured: &dyn Fn(&Element) -> bool,
    vcx: ValueCx<'_>,
) -> Vec<(Element, Shape)> {
    if kids.iter().all(|(c, _)| c.marks.paginate || measured(c)) {
        return kids;
    }
    for _ in 0..6 {
        let probe: Vec<crate::layout::fragment::types::Kid> = kids
            .iter()
            .enumerate()
            .map(|(i, (c, s))| crate::layout::fragment::types::Kid {
                h: s.height,
                mt: s.margin_top,
                mb: s.margin_bottom,
                // Тот же предикат, что у `StackChild` в сборке стопки.
                monolith: solid_box(c),
                cuts: s.cuts.clone(),
                // Щуп обязан видеть ровно то же, что стопка, иначе
                // распорки лягут по другому плану, чем укладка.
                force_before: edge_break(c, false, vcx),
                force_after: edge_break(c, true, vcx),
                // Щуп обязан видеть ровно то же, что стопка: без
                // запретов `growths` считал бы распорки по ДРУГОМУ плану,
                // чем укладка после отступа.
                avoid_before: edge_avoid(c, false, vcx),
                avoid_after: edge_avoid(c, true, vcx),
                forced: s.forced.clone(),
                solid: s.solid.clone(),
                float_min: 0.0,
                avoid: Vec::new(),
                keep: Vec::new(),
                span: c.style.used().column_span == Some(true) && !c.inline,
                // Щуп роста параллельного потока не знает: распорка
                // (`spacer_before`) — про в-поточную высоту, а поток высоты
                // не даёт (css-break-3 §3). Раздвинуть коробку им значило бы
                // вернуть переполнение в поток. `over == 0.0` при `h >= 0`
                // выключает поток в `fill_at` тождественно.
                over: 0.0,
                // Щуп обязан видеть ровно то же, что стопка: без
                // `clone` соседи после такой коробки получили бы распорки по
                // ДРУГОМУ плану.
                clone_dec: clone_dec(c),
                // Тот же предикат, что у `StackChild` в сборке стопки: иначе
                // распорки легли бы по другому плану, чем укладка.
                overflow_top: fixed.is_some()
                    && rows.is_none()
                    && !parallel_items_inside(c, 4, vcx),
                repeat: repeat_leads(c, fixed, rows, ShapeCx::COLUMNS, vcx),
                par: par.get(i).copied().unwrap_or_default(),
            })
            .collect();
        let mut grows = crate::layout::multicol::column_stack::ColumnStack::growths(
            &probe, count, fixed, rows, copies,
        );
        // Внутри ребёнка — снизу вверх: правка ниже точки не сдвигает точки
        // выше, и `at` из одного плана остаётся верным для всех записей
        // прохода.
        grows.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        let mut changed = false;
        for (kid, at, grow, forced) in grows {
            let c = &mut kids[kid].0;
            if c.marks.paginate || measured(c) {
                continue;
            }
            let found = pushed_box_at(c, at, 4, forced, vcx);
            let Some(id) = found else {
                continue;
            };
            // Монолит двигает поле, принудительный разрыв — коробка.
            let moved = if forced {
                spacer_before(c, id, grow, vcx)
            } else {
                grow_before(c, id, grow)
            };
            if !moved {
                continue;
            }
            // TRANSITIONAL(F2-4): После распорки форма ребёнка оценивается заново; удалить в P21 после continuation placement без мутации spacer поверх P05/P08/P10.
            if let Some(s) = shape_full(c, 4, ShapeCx::COLUMNS, vcx) {
                kids[kid].1 = keep_par_margins(s, &kids[kid].1, par.get(kid));
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    kids
}

/// Монолит ребёнка колонок держится ТОЛЬКО на `break-inside: avoid`: ни
/// `contain: size`, ни прокрутки, ни замещаемого, ни атомарной строчной, ни
/// сплошного строчного набора (тот же список, что у `monolith` в сборке
/// стопки, без `break_inside_avoid`).
pub(crate) fn avoid_only_monolith(c: &Element) -> bool {
    let scrolls = |o: Option<crate::style::computed::Overflow>| {
        matches!(o, Some(crate::style::computed::Overflow::Scroll))
    };
    let block_kid = |n: &Node| matches!(n, Node::Element(k) if !k.inline || k.style.used().display == Some(Display::Block));
    !(size_monolith(c)
        || scrolls(c.style.used().overflow_x)
        || scrolls(c.style.used().overflow_y)
        || matches!(
            c.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            c.style.used().display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
        || (c.children.iter().any(|n| !is_blank(n)) && !c.children.iter().any(block_kid)))
}

/// Перемера после распорки — с прежними полями у элемента строки flex
/// (`split_flex_lines` кладёт в поле ещё и `row-gap`).
pub(crate) fn keep_par_margins(
    s: Shape,
    old: &Shape,
    par: Option<&crate::layout::fragment::types::Par>,
) -> Shape {
    if par.is_some_and(|p| p.group != 0) {
        Shape {
            margin_top: old.margin_top,
            margin_bottom: old.margin_bottom,
            ..s
        }
    } else {
        s
    }
}
