//! Whether a root-flow block may be recorded as far (`far_replayable`),
//! decided before its build without allocating.

use crate::dom::{Element, Node};
use crate::layout::reuse::plain;

/// Levels of a subtree the candidacy walk follows on the stack; a block
/// nested deeper is built. The walk runs before every build of a candidate
/// block, so it must cost no allocation: a deeper subtree would need a heap
/// stack in every frame for a block that is most often near the screen.
const DEPTH: usize = 32;

/// The subtree of `e` can be laid out again at another width from its kept
/// layout nodes (`far`): replayable, and only block and inline boxes, which
/// Taffy's block layout and the paragraph's measure function lay out. Tables
/// and inline-level boxes with their own layout (inline-block) are not
/// admitted until their layout at a new width is shown to need no build.
pub(in crate::layout::reuse) fn far_replayable(
    e: &Element,
    vcx: crate::values::ValueCx<'_>,
) -> bool {
    use crate::style::computed::Display;
    within(e, |e| {
        plain::replayable_node(e, vcx)
            && !plain::table(e)
            && matches!(
                e.style.used().reset_box_layout.display,
                None | Some(Display::Block)
            )
    })
}

/// `f` holds for every element of the subtree of `e`, which is at most
/// `DEPTH` levels deep.
fn within(e: &Element, mut f: impl FnMut(&Element) -> bool) -> bool {
    if !f(e) {
        return false;
    }
    let mut open: [std::slice::Iter<'_, Node>; DEPTH] = std::array::from_fn(|_| [].iter());
    open[0] = e.children.iter();
    let mut depth = 1;
    while depth > 0 {
        match open[depth - 1].next() {
            Some(Node::Element(child)) => {
                if !f(child) || depth == DEPTH && !child.children.is_empty() {
                    return false;
                }
                if depth < DEPTH {
                    open[depth] = child.children.iter();
                    depth += 1;
                }
            }
            Some(_) => {}
            None => depth -= 1,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{DEPTH, within};
    use crate::dom::{Element, Marks, Node};
    use crate::style::computed::Computed;

    /// `levels` elements, each the only child of the one before.
    fn nested(levels: usize) -> Element {
        let el =
            |children| Element::synthetic("div", Computed::default(), Marks::default(), children);
        (1..levels).fold(el(vec![]), |inner, _| el(vec![Node::Element(inner)]))
    }

    #[test]
    fn the_candidacy_walk_visits_every_element_up_to_its_depth() {
        let mut seen = 0;
        assert!(within(&nested(DEPTH + 1), |_| {
            seen += 1;
            true
        }));
        assert_eq!(seen, DEPTH + 1);
        assert!(
            !within(&nested(DEPTH + 2), |_| true),
            "a deeper subtree is admitted"
        );
    }
}
