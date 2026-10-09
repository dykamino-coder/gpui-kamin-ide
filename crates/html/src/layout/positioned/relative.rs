//! Относительный сдвиг и подъём абсолютных коробок.
// owner: A

use crate::dom::{Element, Node};
use crate::layout::positioned::static_position::at_static_position;
use crate::render::{is_blank, real_inline};
use crate::style::values::value::Len;

/// Абсолют с краями по ОБЕИМ осям внутри НЕпозиционированного строчного —
/// наружу, соседом этого строчного на блочном уровне. Его содержащий блок —
/// ближайший позиционированный предок (CSS 2.1 §10.1 п.4); прямоугольник
/// фрагментов строчного (§10.1 п.4.1) — только когда позиционирован сам
/// строчный. Куском строки (`Piece::Overlay`) он попадал в нулевую дырку
/// `overlay_in_row` (`inline.rs`), и та становилась его содержащим блоком:
/// `width: 100%` давал ноль, `100px` — коробку 7×7 в углу
/// (`contain-paint-011/012`, `target/dbg/cq-a.html`, `cq-d.html`). Статической
/// позиции у такого абсолюта нет (`at_static_position` ложно), место в
/// потоке ему не нужно. Порядок отрисовки тот же: позиционированные красятся
/// в порядке дерева (прил. E, шаг 8), а вынесенный встаёт сразу за своим
/// строчным.
pub(crate) fn hoist_inset_abs(nodes: &[Node]) -> Vec<Node> {
    fn movable(e: &Element) -> bool {
        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
        matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) && !at_static_position(&e.style)
            && (edge(e.style.inset.left) || edge(e.style.inset.right))
            && (edge(e.style.inset.top) || edge(e.style.inset.bottom))
            && !matches!(
                e.tag.as_str(),
                "input" | "textarea" | "select" | "button" | "img" | "svg" | "canvas"
            )
    }
    // Строчный, сквозь который абсолют уходит: не позиционирован и не заводит
    // содержащего блока иначе (трансформ, `contain`, фильтр — css-transforms-1
    // §3, css-contain-2 §3.2).
    fn passable(e: &Element) -> bool {
        // И без своих НАСЛЕДУЕМЫХ свойств: вынесенный абсолют наследовал бы уже
        // от блока, мимо строчного (`contain-layout-004/005`: `::after` с
        // кеглем Ahem 100px от `rtc` выходил мелким, красное видно).
        let s = &e.style;
        let own_inherited = s.font_size.is_some()
            || s.font_family.is_some()
            || s.font_weight.is_some()
            || s.italic.is_some()
            || s.line_height.is_some()
            || s.color.is_some()
            || s.letter_spacing.is_some()
            || s.word_spacing.is_some()
            || s.text_transform.is_some()
            || s.hidden.is_some()
            || s.rtl.is_some()
            || s.vertical.is_some()
            || s.nowrap.is_some()
            || s.keep_spaces.is_some();
        !own_inherited
            && real_inline(e)
            && e.style.position.is_none()
            && e.style.transform.is_none()
            && e.style.contain_layout != Some(true)
            && e.style.contain_paint != Some(true)
            && !crate::text::inline::establishes_cb(&e.style)
    }
    fn take(e: &mut Element, out: &mut Vec<Node>) {
        let kids = std::mem::take(&mut e.children);
        for n in kids {
            match n {
                Node::Element(k) if movable(&k) => out.push(Node::Element(k)),
                Node::Element(mut k) if passable(&k) => {
                    take(&mut k, out);
                    e.children.push(Node::Element(k));
                }
                other => e.children.push(other),
            }
        }
    }
    fn has(e: &Element) -> bool {
        e.children.iter().any(|n| match n {
            Node::Element(k) => movable(k) || (passable(k) && has(k)),
            _ => false,
        })
    }
    if !nodes
        .iter()
        .any(|n| matches!(n, Node::Element(e) if passable(e) && has(e)))
    {
        return nodes.to_vec();
    }
    let mut out = Vec::with_capacity(nodes.len() + 1);
    for n in nodes {
        match n {
            Node::Element(e) if passable(e) && has(e) => {
                let mut e = e.clone();
                let mut moved = Vec::new();
                take(&mut e, &mut moved);
                out.push(Node::Element(e));
                out.extend(moved);
            }
            other => out.push(other.clone()),
        }
    }
    out
}

/// Сдвиг относительно позиционированной части таблицы.
///
/// Строка и группа строк у нас растворяются в общей сетке — своего элемента
/// у них не остаётся, и `position: relative` вместе с краями пропадал бы
/// молча. Сдвиг переносится на ЯЧЕЙКИ: строка целиком сдвигается ровно
/// настолько же, насколько каждая её ячейка.
///
/// Percentage insets resolve against the parent table part's SPECIFIED size
/// (the table for a row group, the row group for a row), not its used size;
/// an unspecified size makes them `auto` (CSS 2.1 §9.3.2, §10.5; Blink resolves
/// against the parent's percentage-resolution size, crbug.com/1227884,
/// `position-relative-011/012`).
pub(crate) fn relative_shift(e: &Element, parent: Option<&Element>) -> (f32, f32) {
    if e.style.position != Some(crate::style::computed::Position::Relative) {
        return (0.0, 0.0);
    }
    let basis = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let (bw, bh) = parent.map_or((None, None), |p| {
        (basis(p.style.width), basis(p.style.height))
    });
    let len = |l: Option<Len>, base: Option<f32>| match l {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Pct(p)) => base.map(|b| p * b),
        _ => None,
    };
    let side =
        |a: Option<Len>, b: Option<Len>, base: Option<f32>| match (len(a, base), len(b, base)) {
            (Some(v), _) => v,
            // Задан только противоположный край — сдвиг в обратную сторону.
            (_, Some(v)) => -v,
            _ => 0.0,
        };
    (
        side(e.style.inset.left, e.style.inset.right, bw),
        side(e.style.inset.top, e.style.inset.bottom, bh),
    )
}

/// Снять с копии фрагмента ОБЩИЙ относительный сдвиг.
///
/// css-break-3 §5.5: «Fragmentation occurs before relative positioning,
/// transforms, and any other graphical effects. Such effects are applied per
/// fragment». У нас фрагмент — полная копия, обрезанная маской колонки, и
/// сдвиг внутри копии уезжает из маски: содержимое гаснет целиком
/// (`out-of-flow-in-multicolumn-042/045/060/061`; пробы `pm3`, `p042b` —
/// пустой кадр, а `p042` без сдвигов = 0.00). Поэтому сдвиг переносится на
/// ПОСТАНОВКУ фрагмента: и содержимое, и срез едут вместе.
///
/// Спускаемся ПОКА у коробки ровно один непустой ребёнок: только тогда сдвиг
/// заведомо общий для всего, что фрагмент рисует. Складываются лишь точечные
/// края (`Len::Px`) — процентный край остаётся коробке, иначе он пропал бы.
pub(crate) fn hoist_relative(e: &mut Element) -> (f32, f32) {
    let (mut dx, mut dy) = (0.0f32, 0.0f32);
    let mut cur = e;
    loop {
        if cur.style.position == Some(crate::style::computed::Position::Relative) {
            let px_side = |a: Option<Len>, b: Option<Len>| match (a, b) {
                (Some(Len::Px(v)), _) => Some(v),
                (_, Some(Len::Px(v))) => Some(-v),
                _ => None,
            };
            let hx = px_side(cur.style.inset.left, cur.style.inset.right);
            let hy = px_side(cur.style.inset.top, cur.style.inset.bottom);
            if let Some(v) = hx {
                dx += v;
                cur.style.inset.left = None;
                cur.style.inset.right = None;
            }
            if let Some(v) = hy {
                dy += v;
                cur.style.inset.top = None;
                cur.style.inset.bottom = None;
            }
        }
        let mut live = cur
            .children
            .iter()
            .enumerate()
            .filter(|(_, n)| !is_blank(n));
        let i = match (live.next(), live.next()) {
            // Строчная коробка-обёртка (не атомарная) — тоже: её сдвиг
            // относится и к блокам внутри неё (block-in-inline, CSS 2.1
            // §9.2.1.1 — анонимные блоки части этой строчной коробки), а
            // фрагменту он накладывается по css-break-3 §5.5
            // (`out-of-flow-in-multicolumn-058/059`: цепочка `span.rel`).
            (Some((i, Node::Element(k))), None)
                if (!k.inline || k.style.display.is_none())
                    && matches!(
                        k.style.position,
                        None | Some(crate::style::computed::Position::Relative)
                    ) =>
            {
                i
            }
            _ => return (dx, dy),
        };
        match &mut cur.children[i] {
            Node::Element(k) => cur = k,
            _ => return (dx, dy),
        }
    }
}
