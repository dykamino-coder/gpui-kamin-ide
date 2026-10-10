//! Распорки перед перенесённым и рост перенесённой коробки; поля абзаца.

use super::cell::grow_before;
use super::pushed_box_at;
use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::{clone_dec, solid_box};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::{grid_stack, grow_grid_track};
use crate::layout::fragment::probe::size_monolith;
use crate::layout::fragment::push::PUSH_FORCED;
use crate::layout::fragment::table_bands::repeat_leads;
use crate::layout::fragment::{Shape, ShapeCx};
use crate::layout::multicol::spanner::parallel_items_inside;
use crate::render::is_blank;
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Распорка-КОРОБКА перед коробкой `id`: пустой блок высотой `grow`.
/// Нужна принудительному разрыву. Точка `forced` в мере стоит ПЕРЕД
/// схлопнутым полем (`shape_full`: `cuts.push((y, y + lead))`, потом
/// `forced.push(y)`), поэтому `margin-top` её не сдвигает — сдвигает только
/// новый поточный сосед.
pub(super) fn spacer_before(c: &mut Element, id: u64, grow: f32) -> bool {
    // Сетка-стопка: между рядами стоит `row-gap` (`shape_full`:
    // `lead = prev_mb + row_gap + kmt`), и вставка ряда добавляет ЛИШНИЙ
    // зазор. Ряд flex без переноса: высота ряда — `tallest` по детям, а
    // распорка встала бы соседом БОК О БОК и подняла бы весь ряд. Оба
    // случая забирает подъём разрыва (Х5-Х8), а не рост.
    let is_flex = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.flex_dir,
            None | Some(crate::style::computed::FlexDir::Row)
                | Some(crate::style::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap != Some(true)
        && c.style.webkit_box_vertical != Some(true);
    // Зазор гибкой стопки (`row-gap`) встал бы и перед распоркой — лишний
    // зазор, как у сетки-стопки; такой разрыв остаётся без роста.
    let flex_gapped = is_flex
        && c.style.webkit_box != Some(true)
        && matches!(c.style.gap, Some((Some(Len::Px(v)), _)) if v > 0.0);
    let at = if grid_stack(c) || row_nowrap || flex_gapped {
        None
    } else {
        c.children
            .iter()
            .position(|n| matches!(n, Node::Element(k) if k.node_id == id))
    };
    if let Some(i) = at {
        // Распорка двигает только разрыв ПЕРЕД коробкой. Разрыв ПОСЛЕ
        // предыдущего соседа несёт `force_next`, и `forced.push(y)`
        // сработает на границе самой распорки: точка не сдвинется, а у
        // коробки исчезнет вовсе — распорка уедет в следующую колонку
        // вместе с ней. 11 пар из 51 в корзине D держатся только на
        // `break-after`; их забирает подъём (Х5-Х8).
        if !matches!(&c.children[i], Node::Element(k) if edge_break(k, false)) {
            return false;
        }
        // Нижнее поле предыдущего соседа переносится на распорку. Иначе
        // `lead` схлопывается ДВАЖДЫ — перед распоркой (`prev_mb.max(0)`) и
        // перед коробкой (`0.max(kmt)`), — и точка разрыва уезжает на
        // `prev_mb` НИЖЕ края колонки (`trailing-child-margin-000`, `-002`:
        // `margin-bottom: 50px`, обе зелёные).
        let pi = c.children[..i].iter().rposition(|n| !is_blank(n));
        let prev_mb = match pi.map(|j| &c.children[j]) {
            Some(Node::Element(k)) => k.style.margin.bottom,
            _ => None,
        };
        if prev_mb.is_some()
            && let Some(Node::Element(k)) = pi.map(|j| &mut c.children[j])
        {
            k.style.margin.bottom = None;
        }
        let mut style = crate::style::computed::Computed::default();
        style.height = Some(Len::Px(grow));
        style.margin.bottom = prev_mb;
        // Распорка в гибком хозяине — сама элемент: при переносе по строкам
        // она обязана занять СВОЮ строку (иначе встаёт рядом с предыдущим
        // элементом и строку не растит), не сжиматься в контейнере с заданной
        // высотой и стоять в визуальном порядке рядом со своей коробкой
        // (`order`, `reorder` в `blocks()`).
        if matches!(
            c.style.display,
            Some(Display::Flex) | Some(Display::InlineFlex)
        ) {
            style.width = Some(Len::Pct(1.0));
            style.flex_shrink = Some(0.0);
            if let Node::Element(k) = &c.children[i] {
                style.order = k.style.order;
            }
        }
        c.children.insert(
            i,
            Node::Element(Element {
                list_item: None,
                // Свой устойчивый номер: анимации у распорки нет, но номер
                // обязан быть уникальным — иначе GPUI склеит её состояние с
                // коробкой, перед которой она стоит.
                node_id: id ^ 0x5350_4143_4552_0001,
                anim: None,
                tag: "div".to_string(),
                style,
                hover: None,
                first_letter: None,
                first_line: None,
                children: Vec::new(),
                attrs: Vec::new(),
                inline: false,
            }),
        );
        // Заданная высота хозяина СТАРШЕ содержимого (`shape_full`: ветка
        // `c.style.height` возвращает `v + top + bot`), и распорка внутри неё
        // меры не меняет — `changed` не взводится, цикл `grow_pushed` встаёт
        // на первом заходе. Проба `p2-single-line-column-flex-fragmentation-
        // 037` осталась красной именно поэтому, а `p3-…` с высотой 100 → 150
        // сняла 4/5 площади (3906 → 756 точек).
        if let Some(Len::Px(h)) = c.style.height {
            c.style.height = Some(Len::Px(h + grow));
        }
        return true;
    }
    for n in c.children.iter_mut() {
        if let Node::Element(k) = n
            && spacer_before(k, id, grow)
        {
            return true;
        }
    }
    false
}

/// Рост коробки от вытолкнутого монолита (Blink `FinishFragmentation`,
/// `fragmentation_utils.cc:641-656`: у НЕпоследнего фрагмента
/// `final_block_size = space_left`; css-flexbox-1 §fragmentation: «A forced
/// break inside a flex item effectively increases the size of its
/// contents»). Укладка режет ребёнка стопки в НАЧАЛЕ монолитного диапазона
/// `a` (`flow.rs` `fill_at`: `holds` → `at(a)`): в колонке остаётся
/// `a − from` содержимого при остатке `room`, и коробки, где лежит монолит,
/// обязаны дотянуться до низа, а всё после него — сдвинуться на
/// `grow = room − (a − from)`. Копия одна на все колонки, и сдвиг в ней
/// делает распорка — `margin-top += grow` у самой внешней коробки,
/// начинающейся ровно в `a` (`pushed_box_at`). После неё монолит стоит на
/// краю колонки, мера выросла на `grow`, и срез по краю совпадает с Blink:
/// колонка 1 — `room`, следующая копия — с `from + room`. План зависит
/// только от мер (`ColumnStack::growths`), поэтому распорки ставятся ДО
/// сборки копий, до неподвижной точки (≤ 6 заходов; обычно один). Проба
/// устройства руками — `target/probe-9d/*.html`: SLC-007/009/010/011 и
/// `table-cell-expansion-001` 0.00 (`scout-break-2026-09d.md` §3.3).
pub(crate) fn grow_pushed(
    mut kids: Vec<(Element, Shape)>,
    count: usize,
    fixed: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    par: &[crate::layout::fragment::types::Par],
) -> Vec<(Element, Shape)> {
    for _ in 0..6 {
        let probe: Vec<crate::layout::fragment::types::Kid> = kids
            .iter()
            .enumerate()
            .map(|(i, (c, s))| crate::layout::fragment::types::Kid {
                h: s.0,
                mt: s.1,
                mb: s.2,
                // Тот же предикат, что у `StackChild` в сборке стопки.
                monolith: solid_box(c),
                cuts: s.3.clone(),
                // Щуп обязан видеть ровно то же, что стопка (Х6), иначе
                // распорки лягут по другому плану, чем укладка.
                force_before: edge_break(c, false),
                force_after: edge_break(c, true),
                // Щуп обязан видеть ровно то же, что стопка (Х6): без
                // запретов `growths` считал бы распорки по ДРУГОМУ плану,
                // чем укладка после отступа.
                avoid_before: edge_avoid(c, false),
                avoid_after: edge_avoid(c, true),
                forced: s.4.clone(),
                solid: s.5.clone(),
                span: c.style.column_span == Some(true) && !c.inline,
                // Щуп роста параллельного потока не знает: распорка
                // (`spacer_before`) — про в-поточную высоту, а поток высоты
                // не даёт (css-break-3 §3). Раздвинуть коробку им значило бы
                // вернуть переполнение в поток. `over == 0.0` при `h >= 0`
                // выключает поток в `fill_at` тождественно.
                over: 0.0,
                // Щуп обязан видеть ровно то же, что стопка (Х6): без
                // `clone` соседи после такой коробки получили бы распорки по
                // ДРУГОМУ плану.
                clone_dec: clone_dec(c),
                // Тот же предикат, что у `StackChild` в сборке стопки: иначе
                // распорки легли бы по другому плану, чем укладка.
                overflow_top: fixed.is_some() && rows.is_none() && !parallel_items_inside(c, 4),
                repeat: repeat_leads(c, fixed, rows),
                par: par.get(i).copied().unwrap_or_default(),
            })
            .collect();
        let mut grows = crate::layout::multicol::column_stack::ColumnStack::growths(
            &probe, count, fixed, rows, copies,
        );
        // Внутри ребёнка — снизу вверх: правка ниже точки не сдвигает точки
        // выше, и `at` из одного плана остаётся верным для всех записей
        // прохода (рост дорожки иначе находил ряд по устаревшим полосам:
        // `grid-item-fragmentation-048`, два разрыва в одном проходе).
        grows.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal))
        });
        let mut changed = false;
        for (kid, at, grow, forced) in grows {
            let c = &mut kids[kid].0;
            // Ряд в точках, вытолкнутый целиком: растёт ПРЕДЫДУЩАЯ дорожка.
            if grow_grid_track(c, at, grow) {
                if let Some(s) = shape_full(c, 4, ShapeCx::COLUMNS) {
                    kids[kid].1 = keep_par_margins(s, &kids[kid].1, par.get(kid));
                    changed = true;
                }
                continue;
            }
            PUSH_FORCED.with(|f| f.set(forced));
            let found = pushed_box_at(c, at, 4);
            PUSH_FORCED.with(|f| f.set(false));
            let Some(id) = found else {
                continue;
            };
            // Монолит двигает поле, принудительный разрыв — коробка (Х3).
            let moved = if forced {
                spacer_before(c, id, grow)
            } else {
                grow_before(c, id, grow)
            };
            if !moved {
                continue;
            }
            if let Some(s) = shape_full(c, 4, ShapeCx::COLUMNS) {
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
    let block_kid = |n: &Node| matches!(n, Node::Element(k) if !k.inline || k.style.display == Some(Display::Block));
    !(size_monolith(c)
        || scrolls(c.style.overflow_x)
        || scrolls(c.style.overflow_y)
        || matches!(
            c.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            c.style.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
        || (c.children.iter().any(|n| !is_blank(n)) && !c.children.iter().any(block_kid)))
}

/// Перемера после распорки — с прежними полями у элемента строки flex
/// (`split_flex_lines` кладёт в поле ещё и `row-gap`).
pub(super) fn keep_par_margins(
    s: Shape,
    old: &Shape,
    par: Option<&crate::layout::fragment::types::Par>,
) -> Shape {
    if par.is_some_and(|p| p.group != 0) {
        (s.0, old.1, old.2, s.3, s.4, s.5)
    } else {
        s
    }
}
