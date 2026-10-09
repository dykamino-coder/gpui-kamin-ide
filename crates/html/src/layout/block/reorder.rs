//! Переупорядочение детей, процентные инлайн-размеры, ортогональные дети.
// owner: A

use crate::dom::Node;
use crate::layout::table::anon::anon_element;
use crate::layout::writing_mode::orthogonal_fixed_child;
use crate::render::in_flow;
use crate::style::computed::{Align, Computed, Display};
use crate::style::values::value::Len;
use crate::text::text_box::blank_text;

/// `order`: визуальный порядок в гибкой строке.
///
/// Раскладка под нами это свойство не знает, поэтому детей переставляем сами.
/// Сортировка устойчивая — элементы с равным `order` сохраняют порядок
/// разметки, как того требует CSS.
pub(crate) fn reorder(mut nodes: Vec<Node>) -> Vec<Node> {
    let ordered = nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.order.is_some(),
        Node::Text(_) => false,
    });
    if !ordered {
        return nodes;
    }
    // Анонимный элемент — непрерывный прогон текста В ПОРЯДКЕ РАЗМЕТКИ
    // (css-flexbox-1 §4), и `order` переставляет уже готовые элементы. После
    // сортировки прогоны «a a» и «b b» по обе стороны от `order: 1`
    // становились соседями и склеивались в один абзац
    // (`flexbox-anonymous-items-001`). Поэтому при двух и более непустых
    // прогонах каждый заворачивается в свой анонимный блок заранее.
    let blank = |t: &str| blank_text(t);
    let mut runs: Vec<Vec<Node>> = vec![];
    let mut cur: Vec<Node> = vec![];
    let mut rest: Vec<(usize, Node)> = vec![];
    for n in nodes.drain(..) {
        match n {
            Node::Text(_) => cur.push(n),
            other => {
                if !cur.is_empty() {
                    runs.push(std::mem::take(&mut cur));
                    rest.push((usize::MAX, Node::Text(String::new())));
                }
                rest.push((0, other));
            }
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
        rest.push((usize::MAX, Node::Text(String::new())));
    }
    let solid = runs
        .iter()
        .filter(|r| r.iter().any(|n| matches!(n, Node::Text(t) if !blank(t))))
        .count();
    let mut run_iter = runs.into_iter();
    for (tag, n) in rest {
        if tag != usize::MAX {
            nodes.push(n);
            continue;
        }
        let run = run_iter.next().unwrap_or_default();
        let has_text = run.iter().any(|n| matches!(n, Node::Text(t) if !blank(t)));
        if solid >= 2 && has_text {
            nodes.push(Node::Element(anon_element("div", run)));
        } else {
            nodes.extend(run);
        }
    }
    // css-flexbox-1 §5.4 (и css-grid-2 §9.1 по ссылке): «Absolutely-
    // positioned children of a flex container are treated as having
    // order: 0 for the purpose of determining their painting order relative
    // to flex items» — внепоточный ребёнок `order` не слушает, и сортировка
    // оставляет его в порядке разметки среди элементов с нулём
    // (`flexbox-paint-ordering-003`).
    nodes.sort_by_key(|n| match n {
        Node::Element(e)
            if matches!(
                e.style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            ) =>
        {
            0
        }
        Node::Element(e) => e.style.order.unwrap_or(0),
        Node::Text(_) => 0,
    });
    nodes
}

/// Схлопнуть отступы соседей вдоль горизонтальной оси потока.
///
/// Между двумя блоками остаётся больший из смежных отступов, а не их сумма.
/// Раскладка их складывает, поэтому у второго и следующих соседей ведущий
/// отступ уменьшается на уже занятый предыдущим.
// ★ ЗАМЕРЕНО И ОТКАЧЕНО (07.09, v145, `scout-wm-2026-09e.md` E1):
// предел §7.3.2 для ортогонального ребёнка с ЯВНЫМ `width: auto`
// (гейт `width.is_none()` не видел `Len::Auto`) плюс потолок вместо
// жёсткой ширины. Срез css-writing-modes+css-masking+css-shapes+
// filter-effects+css-borders+css-text+CSS2 9052: +1 при −2 —
// `three-levels-of-orthogonal-flows` 0.00 → 16.96,
// `two-levels-of-orthogonal-flows-fixed` 0.09 → 2.91. Вложенные
// ортогональные потоки считают предел от НЕПРАВИЛЬНОГО предка —
// сначала нужен настоящий поиск ближайшего параллельного контейнера.
/// Доли полей и отступов блочных детей — от СТРОЧНОГО размера содержащего
/// блока (css-writing-modes-4 §7.2, Overview.bs:2018-2021: «percentages on
/// the margin and padding properties … are calculated with respect to the
/// inline size of the containing block»). Раскладка под нами (taffy,
/// `compute/flexbox.rs:187/447/723`) берёт базой ВСЕГДА ширину родителя: у
/// вертикального контейнера это блочный размер, при `width: auto` ещё и
/// неопределённый — доли выходили нулём (`percent-padding-vrl-004/006`,
/// `percent-margin-vrl-004`: по расчёту коробки 50×50 вместо 100×70 и
/// 100×139). Переводим в
/// точки ДО схлопывания: `collapse_flow_margins` читает долю через
/// `margin_px` от ФИЗИЧЕСКОЙ ширины.
/// `vertical` — письмо контейнера: база — его высота; у горизонтального
/// контейнера переводятся только вертикальные дети (их проценты taffy считал
/// на проходе с неопределённой шириной — `percent-padding-vrl-002`, отступы
/// сверху/снизу нулём), база — ширина. Сетка исключена: там содержащий блок —
/// область сетки, а не контейнер (css-grid-2, Overview.bs:718: «A grid item’s
/// grid area forms the containing block into which it is laid out»).
pub(crate) fn resolve_inline_pct(
    mut children: Vec<Node>,
    container: &Computed,
    vertical: bool,
) -> Vec<Node> {
    if matches!(
        container.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) {
        return children;
    }
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let b = container.borders();
    let (size, edges) = if vertical {
        (
            container.height,
            [
                b.top,
                b.bottom,
                container.padding.top,
                container.padding.bottom,
            ],
        )
    } else {
        (
            container.width,
            [
                b.left,
                b.right,
                container.padding.left,
                container.padding.right,
            ],
        )
    };
    let Some(mut base) = px(size) else {
        return children;
    };
    // Строчный размер СБ — его content-box: при `border-box` заданная
    // величина включает рамки и отступы по той же оси.
    if container.border_box == Some(true) {
        base -= edges.iter().map(|l| px(*l).unwrap_or(0.0)).sum::<f32>();
    }
    let base = base.max(0.0);
    for node in children.iter_mut() {
        let Node::Element(ch) = node else { continue };
        if ch.inline || !in_flow(&ch.style) {
            continue;
        }
        if !vertical && ch.style.vertical != Some(true) {
            continue;
        }
        let s = &mut ch.style;
        for side in [
            &mut s.margin.top,
            &mut s.margin.right,
            &mut s.margin.bottom,
            &mut s.margin.left,
            &mut s.padding.top,
            &mut s.padding.right,
            &mut s.padding.bottom,
            &mut s.padding.left,
        ] {
            if let Some(Len::Pct(k)) = side {
                *side = Some(Len::Px(*k * base));
            }
        }
    }
    children
}

/// Зеркальный ортогональный случай: ВЕРТИКАЛЬНЫЙ блок внутри горизонтального
/// контейнера. Его строчная ось — высота, и авто-размер по ней зажимается
/// высотой контейнера за вычетом вертикальных полей (css-writing-modes-3
/// §7.3). Доля полей здесь обычная — от ширины контейнера, её решает
/// раскладка сама.
pub(crate) fn orthogonal_vertical_children(children: Vec<Node>, container: &Computed) -> Vec<Node> {
    let has_vertical = children.iter().any(|n| match n {
        Node::Element(ch) => !ch.inline && ch.style.vertical == Some(true),
        _ => false,
    });
    if !has_vertical {
        return children;
    }
    let mut out = children;
    for node in out.iter_mut() {
        let Node::Element(ch) = node else { continue };
        if ch.inline || ch.style.vertical != Some(true) {
            continue;
        }
        if !in_flow(&ch.style) {
            continue;
        }
        // Элемент СЕТКИ с невытягивающим выравниванием: по строчной оси
        // (у него вертикальной) он размером в содержимое, а не в область
        // (css-grid-1 §6.6 вместе с css-align-3 §6.1 — `stretch` растягивает,
        // остальное нет). Пока вертикальный абзац брал весь предел
        // ортогонального потока, эталоны `orthogonal-positioned-grid-items-*`
        // (`place-items: start`) вылезали за сетку на всю высоту окна.
        if matches!(
            container.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) && matches!(
            ch.style.align_self.or(container.align_items),
            Some(Align::Start) | Some(Align::Center) | Some(Align::End) | Some(Align::Baseline)
        ) {
            ch.style.hug_inline = true;
        }
        // Корень с vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
        // flow). Прижим самим стилем корня (align-self) — контейнеры-колонки
        // его уважают; для корня с ФОНОМ-КАРТИНКОЙ якорь ранее гасил
        // canvas-слой — тем страницам якорь не ставится (замерено).
        if matches!(ch.tag.as_str(), "html" | "body")
            && ch.style.vertical_rl == Some(true)
            && ch.style.align_self.is_none()
            && ch.style.bg_image.is_none()
        {
            ch.style.align_self = Some(crate::style::computed::Align::End);
        }
        // Ordinary orthogonal blocks now compute their own used inline size.
        if orthogonal_fixed_child::normal_block_flow(&ch.style, container) {
            ch.style.flex_shrink = Some(0.0);
            continue;
        }
        if ch.style.height.is_some() || ch.style.max_height.is_some() {
            continue;
        }
        let margin = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) => match container.width {
                Some(Len::Px(w)) => k * w,
                _ => 0.0,
            },
            _ => 0.0,
        };
        let margins = margin(ch.style.margin.top) + margin(ch.style.margin.bottom);
        ch.style.max_height = Some(match container.height {
            Some(Len::Px(h)) => Len::Px((h - margins).max(0.0)),
            _ => Len::Pct(1.0),
        });
        // Тот же предел — и ПЕРЕНОСУ строк. Авто-строчный размер ортогонального
        // блока — shrink-to-fit к размеру, что «would stretch fit into … the
        // containing block’s size if that is fixed» (css-writing-modes-4
        // §7.3.2, Overview.bs:2175-2183), а stretch-fit вычитает поля ребёнка.
        // Blink: `length_utils.cc:117-146` (`kFitContent` →
        // `ShrinkToFit(available_size - margins.InlineSum())`), авто-длина
        // ортогонального ребёнка — `FitContent` (`length_utils.cc:555-569`).
        // `max_height` выше этого не даёт: вето `element()` «предок уже дал
        // предел» у вертикального блока без своей `height` оставляет предел
        // КОНТЕЙНЕРА, и текст переносился по 200 вместо 200 − 2·50
        // (`sizing-orthogonal-percentage-margin-001/002`: эталон с `height:
        // 100px` после сужения вето переносит по 100, тест — по 200).
        // Свой `ortho_limit` перебивает унаследованный при слиянии
        // (`inline.rs`: `own.ortho_limit.or(parent.ortho_limit)`); рамки и
        // отбивки ребёнка из него вычтет `element()`. Только блочный
        // контейнер (у сетки/гибкого содержащий блок другой) и только при
        // ненулевых полях — без них предел равен унаследованному.
        if let Some(Len::Px(h)) = container.height
            && margins > 0.0
            && matches!(container.display, None | Some(Display::Block))
        {
            let px = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            // Предел — внутренний размер СБ: при `border-box` заданная
            // высота включает его рамки и отбивки по той же оси.
            let inner = if container.border_box == Some(true) {
                let b = container.borders();
                h - px(b.top)
                    - px(b.bottom)
                    - px(container.padding.top)
                    - px(container.padding.bottom)
            } else {
                h
            };
            ch.style.ortho_limit = Some((inner - margins).max(0.0));
        }
    }
    out
}
