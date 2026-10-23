//! Схлопывание вертикальных полей блочного потока (CSS 2.1 §8.3.1) перед раскладкой.
//! Вход: дети блока; выход — копия детей, у которых поля уже сведены к тому, что должна
//! сложить раскладка: подъём полей детей в родителя (`kids`), струна соседей (`struts`),
//! нулевая высота блока из одних флоатов (§10.6.3). Поля, заданные в относительных единицах,
//! решаются заранее по кеглю, ширине и определённости высоты содержащего блока
//! (`CollapseInputs`).

use crate::dom::Node;
use crate::layout::block::margin_inline_boxes;
use crate::layout::block::struts::{Strut, solve, through_strut_no_clear};
use crate::style::values::value::Len;
mod inputs;
pub(crate) use inputs::CollapseInputs;
mod flow;
pub(super) use flow::collapse_flow_margins;
mod kids;
use kids::{collapse_kid_margins, emit_margin_struts};

/// Поля отдельной копии до корневой раскладки: поля, примыкающие к детям,
/// принадлежат внешнему потоку, а не корню Taffy (CSS 2.1 §8.3.1).
pub(crate) fn fragment_box(
    element: crate::dom::Element,
    width: f32,
    vcx: crate::document::state::ValueCx<'_>,
) -> crate::dom::Element {
    let vcx = vcx.with_collapse(CollapseInputs {
        width: Some(width),
        ..vcx.collapse
    });
    let mut nodes = [Node::Element(element)];
    collapse_kid_margins(&mut nodes, Some(width), vcx);
    let [Node::Element(mut element)] = nodes else {
        unreachable!("margin collapse preserves the box")
    };
    crate::layout::block::struts::pin_inherited_margins(&mut element, true, true, vcx);
    let lengths = crate::style::computed::LenCx {
        basis: Some(width),
        ..crate::style::computed::LenCx::box_viewport(Some(vcx.viewport), &element.style.computed())
    };
    let used = element.style.used_mut();
    for margin in [&mut used.margin.top, &mut used.margin.bottom] {
        if let Some(px) = lengths.px_strict(*margin, vcx) {
            *margin = Some(Len::Px(px));
        }
    }
    element
}

/// Поля отдельной вертикальной копии: тот же подъём, что в общем потоке.
pub(crate) fn vertical_fragment_box(
    mut element: crate::dom::Element,
    width: f32,
    reverse: bool,
    vcx: crate::document::state::ValueCx<'_>,
) -> crate::dom::Element {
    let vcx = vcx.with_collapse(CollapseInputs {
        width: Some(width),
        ..vcx.collapse
    });
    flow::absorb_margin(&mut element, false, reverse, vcx);
    flow::absorb_margin(&mut element, true, reverse, vcx);
    let lengths = crate::style::computed::LenCx {
        basis: Some(width),
        ..crate::style::computed::LenCx::box_viewport(Some(vcx.viewport), &element.style.computed())
    };
    let used = element.style.used_mut();
    for margin in [&mut used.margin.left, &mut used.margin.right] {
        if let Some(px) = lengths.px_strict(*margin, vcx) {
            *margin = Some(Len::Px(px));
        }
    }
    element
}

/// Поля отдельной копии: нерешённая длина требует содержащего блока.
pub(crate) fn fragment_margins(c: &crate::dom::Element) -> Option<(f32, f32)> {
    let m = |l: &Option<Len>| match l {
        Some(Len::Px(v)) => Some(*v),
        None | Some(Len::Auto) => Some(0.0),
        _ => None,
    };
    Some((
        m(&c.style.computed().margin.top)?,
        m(&c.style.computed().margin.bottom)?,
    ))
}

/// Схлопывание вертикальных отступов соседних блоков.
///
/// В CSS нижний отступ одного блока и верхний отступ следующего не
/// складываются, а сливаются в больший из двух. Движок раскладки под нами
/// складывает их, и документ становится длиннее браузерного — расхождение
/// накапливается сверху вниз и было поймано сравнением с Chrome.
/// `abs_parent` — родитель абсолютно позиционирован: по §10.6.7 его
/// автовысота ВКЛЮЧАЕТ плавающих детей, и правило «блок из одних флоатов
/// высотой ноль» (§10.6.3) к его детям не применяется. `cb` — ширина
/// содержащего блока в точках (`BoxConstraints::cb_width`): по ней цепочка решает,
/// влезает ли коробка своего контекста рядом с флоатами (`bfc_no_fit`).
pub(crate) fn collapse_margins(
    nodes: &[Node],
    abs_parent: bool,
    cb: Option<f32>,
    vcx: crate::document::state::ValueCx<'_>,
) -> Vec<Node> {
    let mut out: Vec<Node> = nodes.to_vec();
    margin_inline_boxes::prepare(&mut out);
    // CSS 2.1 §10.6.3: floats do not contribute to ordinary auto height.
    // Margin collapse proves zero in-flow height for an open empty block;
    // The contextual proof also handles borders, padding and white-space.
    // Preserve computed auto for percentage and orthogonal sizing (§10.5);
    // only styled_div applies the proven used zero via margin_height::used_style.
    // Formatting contexts retain floats (§10.6.7). Keep the absolute-parent
    // guard: its float containment currently depends on the child's height.
    for node in out.iter_mut().filter(|_| !abs_parent) {
        let Node::Element(e) = node else { continue };
        let has_float = e.children.iter().any(
            |n| matches!(n, Node::Element(c) if c.style.computed().float.is_some_and(|f| f != 0)),
        );
        if has_float && through_strut_no_clear(e, vcx).is_some() {
            e.marks.auto_zero_height = true;
        }
    }
    // Отступ первого ребёнка «протекает» наружу, если родителя от него не
    // отделяют ни рамка, ни внутренний отступ: в CSS это один и тот же отступ,
    // а не два. Без этого блок уезжает вниз на величину детского отступа.
    collapse_kid_margins(&mut out, cb, vcx);
    // Струна примыкающих полей соседей (§8.3.1). `emitted` — сколько точек уже
    // ЗАПИСАНО в стили этого зазора: раскладка складывает поля сама, и
    // верхнему полю следующего блока достаётся только разница.
    let mut strut: Option<Strut> = None;
    let mut emitted = 0.0f32;
    // Последняя коробка прогона с клиренсом: её остаток поля остаётся ВНУТРИ
    // родителя и наружу не уходит.
    let mut cleared_run: Option<usize> = None;
    emit_margin_struts(&mut out, &mut strut, &mut emitted, &mut cleared_run, vcx);
    // Прогон кончился на коробке с клиренсом: остаток слитого поля пишется ей
    // самой — родителя он растит, но наружу не выходит.
    if let (Some(i), Some(s)) = (cleared_run, strut) {
        let rest = solve(s) - emitted;
        if rest > 0.0
            && let Some(Node::Element(e)) = out.get_mut(i)
        {
            e.style.used_mut().margin.bottom = Some(Len::Px(rest));
        }
    }
    out
}

#[cfg(test)]
mod tests;
