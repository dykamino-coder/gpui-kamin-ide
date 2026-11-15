//! Границы анонимных фрагментов: обход только поддерева анонимной коробки,
//! без захода в авторские элементы.

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
    /// Первый и последний ключ источника внутри элемента `e`: авторский
    /// элемент даёт свой ключ, текст — ключ своего узла, анонимный — свои.
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
