//! Float tails follow only the last in-flow block chain of an unbordered root.
//! CSS 2.1 10.6.3: floats may escape auto height only when no following box in
//! this formatting context can observe them. Orthogonal and item boxes stop it.
use crate::budget::admission::{Admission, Error};
use crate::dom::normalization::{Transform, apply};
use crate::dom::{Children, Element, Node};
use crate::style::{computed::Display, values::value::Len};

pub(super) fn mark(admission: &Admission, nodes: Vec<Node>) -> Result<Vec<Node>, Error> {
    apply(&FloatTail, nodes, TailContext::default(), admission)
}

pub(crate) struct FloatTail;
#[derive(Clone, PartialEq)]
pub(crate) struct TailContext {
    root: bool,
    vertical: bool,
    item_parent: bool,
}
impl Default for TailContext {
    fn default() -> Self {
        Self {
            root: true,
            vertical: false,
            item_parent: false,
        }
    }
}
impl Transform for FloatTail {
    type Context = TailContext;

    fn visit_child(&self, nodes: &Children, index: usize, parent: &TailContext) -> bool {
        let Node::Element(child) = &nodes[index] else {
            return false;
        };
        if parent.root {
            let b = child.style.cascaded().borders();
            return block_like(child)
                && [b.top, b.right, b.bottom, b.left]
                    .iter()
                    .all(|w| matches!(w, None | Some(Len::Px(0.0))));
        }
        if parent.item_parent || !block_like(child) {
            return false;
        }
        // Each rejected sibling stops at the next nonblank node: the total
        // scan is linear, including long whitespace runs between elements.
        nodes[index + 1..]
            .iter()
            .all(|n| matches!(n, Node::Text(t) if crate::dom::whitespace::blank_text(t)))
            && child
                .style
                .cascaded()
                .vertical_decl()
                .unwrap_or(parent.vertical)
                == parent.vertical
            && !crate::style::computed::formatting_context::own_context_style(
                child.style.cascaded(),
            )
            && child
                .style
                .cascaded()
                .reset_box_layout
                .float
                .is_none_or(|f| f == 0)
    }

    fn enter(
        &self,
        child: &mut Element,
        parent: &TailContext,
        _: &Admission,
    ) -> Result<TailContext, Error> {
        child.style.cascaded_mut().reset_context.float_tail = true;
        let st = child.style.cascaded();
        Ok(TailContext {
            root: false,
            vertical: if parent.root {
                st.wm().vertical()
            } else {
                st.vertical_decl().unwrap_or(parent.vertical)
            },
            item_parent: matches!(
                st.reset_box_layout.display,
                Some(
                    Display::Flex
                        | Display::InlineFlex
                        | Display::Grid
                        | Display::InlineGrid
                        | Display::GridLanes
                )
            ),
        })
    }
}

fn block_like(e: &Element) -> bool {
    match e.style.cascaded().reset_box_layout.display {
        Some(Display::Block | Display::ListItem) => true,
        Some(_) => false,
        None => !e.inline,
    }
}
