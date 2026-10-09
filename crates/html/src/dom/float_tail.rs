//! Blocks whose floats cannot reach anything after them.
//!
//! CSS 2.1 §10.6.3: an ordinary block's auto height counts only in-flow
//! boxes; its floats still belong to the enclosing block formatting context,
//! where they push later clearance, shorten later line boxes and grow the
//! auto height of the formatting-context root (§9.5, §10.6.7). Our float host
//! is laid out inside the block, so it may drop floats from the block's height
//! only when nothing later in that formatting context can observe them: the
//! block and every ancestor up to the root are the last child of their parent,
//! none of them establishes a formatting context, and the root has no border
//! that would show its float-containing height.

use crate::dom::Node;
use crate::style::computed::Display;
use crate::style::values::value::Len;

pub(super) fn mark(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(root) = node else { continue };
        let b = root.style.borders();
        let bare = [b.top, b.right, b.bottom, b.left]
            .iter()
            .all(|w| matches!(w, None | Some(Len::Px(0.0))));
        if bare && block_like(root) {
            root.style.float_tail = true;
            let vertical = root.style.vertical.unwrap_or(false);
            descend(root, vertical);
        }
    }
}

fn descend(parent: &mut crate::dom::Element, vertical: bool) {
    let item_parent = matches!(
        parent.style.display,
        Some(
            Display::Flex
                | Display::InlineFlex
                | Display::Grid
                | Display::InlineGrid
                | Display::GridLanes
        )
    );
    let last = parent.children.iter().rposition(|n| match n {
        Node::Text(t) => !t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')),
        Node::Element(_) => true,
    });
    let Some(last) = last else { return };
    let Node::Element(child) = &mut parent.children[last] else {
        return;
    };
    let child_vertical = child.style.vertical.unwrap_or(vertical);
    // Writing Modes 3 §3.2: a box orthogonal to its parent establishes an
    // independent formatting context, as do flex and grid items.
    if item_parent
        || child_vertical != vertical
        || !block_like(child)
        || crate::render::own_context_style(&child.style)
        || child.style.float.is_some_and(|f| f != 0)
    {
        return;
    }
    child.style.float_tail = true;
    descend(child, child_vertical);
}

fn block_like(e: &crate::dom::Element) -> bool {
    match e.style.display {
        Some(Display::Block | Display::ListItem) => true,
        Some(_) => false,
        None => !e.inline,
    }
}
