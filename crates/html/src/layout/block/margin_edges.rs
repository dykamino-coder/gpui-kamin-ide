//! Схлопывание поля родителя с цепочкой полей детей по верхнему и нижнему краю отдельно
//! (CSS 2.1 §8.3.1). Верхний край: поле родителя и вся ведущая цепочка потомков — одно поле.
//! Нижний край: хвостовая цепочка примыкающих пустых братьев; заданная высота, `min-height`,
//! закрытый край, клиренс и обёртка из одних флоатов спуск останавливают. Поглощённые поля
//! детей обнуляются по пути (`zero_paths`), слитое поле пишется родителю.

use crate::dom::whitespace::blank_text;
use crate::dom::{Element, Node};
use crate::layout::block::containing::with_inner_cb;
use crate::layout::block::margin_height;
use crate::layout::block::struts::{
    Strut, ThroughCache, ZeroPaths, adjoin, float_only_wrapper, leading_chain, margin_or_bail,
    pin_inherited_margins, solve, strut_of, top_edge_open_at, zero_len, zero_paths,
};
use crate::layout::box_kind::{in_flow, inline_level_box, own_context, phantom_inline};
use crate::style::values::value::Len;

pub(super) fn collapse_top(
    e: &mut Element,
    cb: Option<f32>,
    vcx: crate::document::state::ValueCx<'_>,
) {
    // CSS 2.1 section 8.3.1: a top border/padding prevents only top
    // parent-child collapse; bottom collapse has its own edge conditions.
    if !top_edge_open_at(e, vcx.collapse.width) {
        return;
    }
    // Верхнее поле родителя и ВСЯ ведущая цепочка полей потомков — одно
    // поле (§8.3.1). Прежде поднималось поле ровно ОДНОГО ребёнка, и на
    // следующем уровне то же поле внука поднималось повторно.
    let mut eat = ZeroPaths::default();
    // Дети `e` лежат в `e`: его ширина точками — содержащий блок для
    // `bfc_no_fit` внутри цепи.
    let cb = match e.style.used().width {
        Some(Len::Px(w)) if w > 0.0 => Some(w),
        _ => cb,
    };
    let chain = with_inner_cb(&e.style.used(), vcx, |vcx| {
        leading_chain(&e.children, &mut eat, cb, vcx)
    });
    if let (Some(s), Some(own)) = (
        chain,
        margin_or_bail(e.style.used().margin.top, &e.style.used(), vcx),
    ) && !eat.is_empty()
    {
        pin_inherited_margins(e, true, false, vcx);
        e.style.used_mut().margin.top = Some(Len::Px(solve(adjoin(strut_of(own), s))));
        zero_paths(&mut e.children, &eat, true, vcx);
    }
}

pub(super) fn collapse_bottom(e: &mut Element, vcx: crate::document::state::ValueCx<'_>) {
    let mut eat = ZeroPaths::default();
    let chain = with_inner_cb(&e.style.used(), vcx, |vcx| {
        bottom_chain(&e.children, &mut eat, vcx)
    });
    let Some(chain) = chain else { return };
    let Some(own) = margin_or_bail(e.style.used().margin.bottom, &e.style.used(), vcx) else {
        return;
    };
    pin_inherited_margins(e, false, true, vcx);
    e.style.used_mut().margin.bottom = Some(Len::Px(solve(adjoin(strut_of(own), chain))));
    zero_paths(&mut e.children, &eat, false, vcx);
}

/// CSS 2.1 section 8.3.1: adjoining empty siblings extend the end strut,
/// but a fixed height, minimum height or closed bottom edge stops descent.
fn bottom_chain(
    children: &[Node],
    eat: &mut ZeroPaths,
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<Strut> {
    let mut cache = ThroughCache::default();
    let mut parent = 0;
    let mut children = children;
    let mut vcx = vcx;
    let mut strut = strut_of(0.0);
    'level: loop {
        for (i, node) in children.iter().enumerate().rev() {
            let child = match node {
                Node::Text(text) if blank_text(text) => continue,
                Node::Text(_) => return Some(strut),
                Node::Element(child) => child,
            };
            // Preserve the existing static-position anchor boundary.
            if !in_flow(&child.style.used()) {
                return Some(strut);
            }
            if inline_level_box(child) {
                if phantom_inline(node) {
                    continue;
                }
                return Some(strut);
            }
            // Clearance adjoining a self-collapsing child's end margin keeps
            // the entire strut inside its parent (CSS 2.1 section 8.3.1).
            if child.style.used().clear.is_some() && cache.get(child, true, vcx).is_some() {
                return None;
            }
            // A self-collapsing wrapper holding only floats places them at its
            // top border edge, i.e. after the margins that precede it (CSS 2.1
            // section 8.3.1: "as if the element had a non-zero bottom border").
            // Lifting those earlier margins into the parent's end margin would
            // pull the floats up by them; the chain ends at the wrapper.
            if float_only_wrapper(child).is_some() {
                return Some(strut);
            }

            strut = adjoin(
                strut,
                strut_of(margin_or_bail(
                    child.style.used().margin.bottom,
                    &child.style.used(),
                    vcx,
                )?),
            );
            // A self-collapsing box that anchors floats keeps its top margin: its
            // floats sit at the position its top border edge would have with a
            // non-zero bottom border (CSS 2.1 section 8.3.1), after its top margin
            // collapsed with the preceding sibling's. Eating that margin raised
            // the floats by it (`c414-flt-fit-002`); only its end margin joins
            // the parent's, as for a box that does not collapse through.
            if let Some(through) = cache
                .get(child, false, vcx)
                .filter(|_| !anchors_floats(&child.children))
            {
                strut = adjoin(strut, through);
                eat.add(parent, i, true);

                continue;
            }
            let next = eat.add(parent, i, false);
            if !own_context(child)
                && zero_len(child.style.used().padding.bottom)
                && zero_len(child.style.used().borders().bottom)
                && zero_len(child.style.used().min_height)
                && matches!(child.style.used().height, None | Some(Len::Auto))
                && !margin_height::lowers(child, vcx)
                && child.style.used().margin_trim & 2 == 0
            {
                vcx = with_inner_cb(&child.style.used(), vcx, |inner| inner);
                children = &child.children;
                parent = next;
                continue 'level;
            }

            return Some(strut);
        }
        return Some(strut);
    }
}

/// Whether a float is laid out inside these nodes' block formatting context.
fn anchors_floats(children: &[Node]) -> bool {
    let mut pending = vec![children];
    while let Some(children) = pending.pop() {
        for node in children {
            let Node::Element(ch) = node else { continue };
            if ch.style.used().float.is_some_and(|f| f != 0) {
                return true;
            }
            if in_flow(&ch.style.used()) && !own_context(ch) {
                pending.push(&ch.children);
            }
        }
    }
    false
}
