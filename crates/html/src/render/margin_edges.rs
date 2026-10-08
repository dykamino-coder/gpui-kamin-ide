//! Collapse the block-start edge independently of the block-end edge.

use super::*;

pub(super) fn collapse_top(e: &mut Element) {
    // CSS 2.1 section 8.3.1: a top border/padding prevents only top
    // parent-child collapse; bottom collapse has its own edge conditions.
    if !top_edge_open(e) {
        return;
    }
    // Верхнее поле родителя и ВСЯ ведущая цепочка полей потомков — одно
    // поле (§8.3.1). Прежде поднималось поле ровно ОДНОГО ребёнка, и на
    // следующем уровне то же поле внука поднималось повторно.
    let mut path: Vec<usize> = vec![];
    let mut eat: Vec<(Vec<usize>, bool)> = vec![];
    // Дети `e` лежат в `e`: его ширина точками — содержащий блок для
    // `bfc_no_fit` внутри цепи.
    let cb_prev = CB_WIDTH.get();
    if let Some(Len::Px(w)) = e.style.width
        && w > 0.0
    {
        CB_WIDTH.set(Some(w));
    }
    let chain = with_inner_cb(&e.style, || leading_chain(&e.children, &mut path, &mut eat));
    CB_WIDTH.set(cb_prev);
    if let (Some(s), Some(own)) = (chain, margin_or_bail(e.style.margin.top, &e.style))
        && !eat.is_empty()
    {
        pin_inherited_margins(e, true, false);
        e.style.margin.top = Some(Len::Px(solve(adjoin(strut_of(own), s))));
        for (p, deep) in &eat {
            zero_at(&mut e.children, p, true, *deep);
        }
    }
}

pub(super) fn collapse_bottom(e: &mut Element) {
    let mut path = Vec::new();
    let mut eat = Vec::new();
    let chain = with_inner_cb(&e.style, || bottom_chain(&e.children, &mut path, &mut eat));
    let Some(chain) = chain else { return };
    let Some(own) = margin_or_bail(e.style.margin.bottom, &e.style) else {
        return;
    };
    pin_inherited_margins(e, false, true);
    e.style.margin.bottom = Some(Len::Px(solve(adjoin(strut_of(own), chain))));
    for (path, deep) in eat {
        zero_at(&mut e.children, &path, false, deep);
    }
}

/// CSS 2.1 section 8.3.1: adjoining empty siblings extend the end strut,
/// but a fixed height, minimum height or closed bottom edge stops descent.
fn bottom_chain(
    children: &[Node],
    path: &mut Vec<usize>,
    eat: &mut Vec<(Vec<usize>, bool)>,
) -> Option<Strut> {
    let mut strut = strut_of(0.0);
    for (i, node) in children.iter().enumerate().rev() {
        let child = match node {
            Node::Text(text) if blank_text(text) => continue,
            Node::Text(_) => return Some(strut),
            Node::Element(child) => child,
        };
        // Preserve the existing static-position anchor boundary.
        if !in_flow(&child.style) {
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
        if child.style.clear.is_some() && through_strut_no_clear(child).is_some() {
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
        path.push(i);
        strut = adjoin(
            strut,
            strut_of(margin_or_bail(child.style.margin.bottom, &child.style)?),
        );
        // A self-collapsing box that anchors floats keeps its top margin: its
        // floats sit at the position its top border edge would have with a
        // non-zero bottom border (CSS 2.1 section 8.3.1), after its top margin
        // collapsed with the preceding sibling's. Eating that margin raised
        // the floats by it (`c414-flt-fit-002`); only its end margin joins
        // the parent's, as for a box that does not collapse through.
        if let Some(through) = through_strut(child).filter(|_| !anchors_floats(&child.children)) {
            strut = adjoin(strut, through);
            eat.push((path.clone(), true));
            path.pop();
            continue;
        }
        eat.push((path.clone(), false));
        if !own_context(child)
            && zero_len(child.style.padding.bottom)
            && zero_len(child.style.borders().bottom)
            && zero_len(child.style.min_height)
            && matches!(child.style.height, None | Some(Len::Auto))
            && !margin_height::lowers(child)
            && child.style.margin_trim & 2 == 0
        {
            strut = adjoin(
                strut,
                with_inner_cb(&child.style, || bottom_chain(&child.children, path, eat))?,
            );
        }
        path.pop();
        return Some(strut);
    }
    Some(strut)
}

/// Whether a float is laid out inside these nodes' block formatting context.
fn anchors_floats(children: &[Node]) -> bool {
    children.iter().any(|node| match node {
        Node::Element(ch) => {
            ch.style.float.is_some_and(|f| f != 0)
                || (in_flow(&ch.style) && !own_context(ch) && anchors_floats(&ch.children))
        }
        Node::Text(_) => false,
    })
}
