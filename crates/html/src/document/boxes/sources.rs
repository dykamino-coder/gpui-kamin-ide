//! Bounds of anonymous fragments: a walk of the anonymous box's subtree only,
//! without entering authored elements.

use super::source::Source;
use crate::dom::{Element, Node, NodeKey};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) struct Extent {
    first: Option<NodeKey>,
    last: Option<NodeKey>,
}

impl Extent {
    fn node(key: NodeKey) -> Self {
        Self {
            first: Some(key),
            last: Some(key),
        }
    }

    fn append(&mut self, next: Self) {
        self.first = self.first.or(next.first);
        self.last = next.last.or(self.last);
    }
}

impl Extent {
    /// The first and last source key inside element `e`: an authored
    /// element gives its own key, a text the key of its node, an anonymous one its own.
    pub(super) fn of(
        e: &Element,
        text_keys: &[NodeKey],
        element_key: &dyn Fn(u64) -> Option<NodeKey>,
    ) -> Self {
        let mut out = Self::default();
        let mut pending = vec![e.children.iter()];
        while let Some(siblings) = pending.last_mut() {
            match siblings.next() {
                Some(Node::Text(text)) => {
                    if let Some(key) = text
                        .source()
                        .and_then(|id| text_keys.get(id.get() as usize - 1))
                    {
                        out.append(Self::node(*key));
                    }
                }
                Some(Node::Element(child)) => {
                    if Source::authored(child)
                        && let Some(key) = element_key(child.node_id)
                    {
                        out.append(Self::node(key));
                    } else {
                        pending.push(child.children.iter());
                    }
                }
                None => {
                    pending.pop();
                }
            }
        }
        out
    }
}
