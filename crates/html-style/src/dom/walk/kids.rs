//! Список детей бокса во время обхода: ёмкость берётся при первом ребёнке.
//!
//! Оценка `kept_children_hint` известна уже на входе в элемент, но память под
//! список занимается только когда первый ребёнок готов. Иначе пустые списки
//! всей цепочки предков держали бы резерв, пока стилизуется самое глубокое
//! поддерево, и поднимали пик фазы стиля. Детей, выброшенных каскадом
//! (`display: none`, `script`), оценка не видит, поэтому резерв до первого
//! ребёнка был бы ещё и лишним.

use crate::dom::Node;
use markup5ever_rcdom::Handle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Default)]
pub(super) struct Kids {
    nodes: Vec<Node>,
    reserve: usize,
}

impl Kids {
    /// Пустой список с отложенным резервом под детей `handle`, которые
    /// переживут обработку пробелов.
    pub fn kept(handle: &Handle, hidden: bool) -> Self {
        Self {
            nodes: Vec::new(),
            reserve: super::text_nodes::kept_children_hint(handle, hidden),
        }
    }

    /// Список, уже начатый вызывающим: растёт как обычный `Vec`.
    pub fn from_vec(nodes: Vec<Node>) -> Self {
        Self { nodes, reserve: 0 }
    }

    pub fn into_vec(self) -> Vec<Node> {
        self.nodes
    }

    fn reserve_first(&mut self) {
        if self.nodes.capacity() == 0 {
            self.nodes.reserve_exact(self.reserve);
        }
    }

    pub fn push(&mut self, node: Node) {
        self.reserve_first();
        self.nodes.push(node);
    }

    pub fn extend(&mut self, nodes: impl IntoIterator<Item = Node>) {
        let mut nodes = nodes.into_iter().peekable();
        if nodes.peek().is_some() {
            self.reserve_first();
            self.nodes.extend(nodes);
        }
    }
}

impl Deref for Kids {
    type Target = Vec<Node>;
    fn deref(&self) -> &Vec<Node> {
        &self.nodes
    }
}

impl DerefMut for Kids {
    fn deref_mut(&mut self) -> &mut Vec<Node> {
        &mut self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::Kids;
    use html5ever::tendril::TendrilSink;

    #[test]
    fn reserve_waits_for_the_first_child() {
        let dom =
            html5ever::parse_document(markup5ever_rcdom::RcDom::default(), Default::default())
                .one("<body><p>a</p><p>b</p><p>c</p></body>");
        let html = dom.document.children.borrow()[0].clone();
        let body = html.children.borrow()[1].clone();
        let mut kids = Kids::kept(&body, false);
        assert_eq!(kids.capacity(), 0);
        kids.extend(std::iter::empty());
        assert_eq!(kids.capacity(), 0);
        kids.push(crate::dom::Node::Text(crate::dom::Text::from(" ")));
        assert_eq!(kids.capacity(), 3);
    }
}
