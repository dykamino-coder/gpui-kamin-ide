//! Границы анонимных фрагментов: один проход входа, без запроса DOM-поддеревьев.

use super::source::Source;
use crate::dom::{Element, Node, NodeKey};
use std::collections::HashMap;

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

#[derive(Default)]
pub(super) struct Sources(HashMap<usize, Extent>);

impl Sources {
    pub(super) fn collect(
        nodes: &[Node],
        text_keys: &[NodeKey],
        element_key: &dyn Fn(u64) -> Option<NodeKey>,
    ) -> Self {
        let mut out = Self::default();
        let mut pending = vec![(nodes.iter(), None::<&Element>, Extent::default())];
        while let Some((siblings, _, extent)) = pending.last_mut() {
            match siblings.next() {
                Some(Node::Text(text)) => {
                    if let Some(key) = text
                        .source()
                        .and_then(|id| text_keys.get(id.get() as usize - 1))
                    {
                        extent.append(Extent::node(*key));
                    }
                }
                Some(Node::Element(element)) => {
                    pending.push((element.children.iter(), Some(element), Extent::default()));
                }
                None => {
                    let (_, element, mut extent) = pending.pop().expect("source frame");
                    if let Some(element) = element {
                        if Source::authored(element)
                            && let Some(key) = element_key(element.node_id)
                        {
                            extent = Extent::node(key);
                        } else {
                            out.0.insert(element as *const Element as usize, extent);
                        }
                    }
                    if let Some((_, _, parent)) = pending.last_mut() {
                        parent.append(extent);
                    }
                }
            }
        }
        out
    }

    pub(super) fn get(&self, element: &Element) -> Extent {
        self.0
            .get(&(element as *const Element as usize))
            .copied()
            .unwrap_or_default()
    }
}
