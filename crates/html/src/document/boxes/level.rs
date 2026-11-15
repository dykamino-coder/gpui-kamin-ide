//! Один уровень обхода публикации: новые братья, их прежние коробки и сравнение
//! с прежним входом.

use super::BoxTree;
use super::reuse::Reuse;
use super::source::Source;
use crate::dom::{BoxId, Element, Node};

/// С чем сравниваются братья уровня обхода.
#[derive(Clone, Copy)]
pub(super) enum Prev<'a> {
    /// Общий снимок с прежним входом: поддерево не изменилось.
    Clean,
    /// Прежние братья того же родителя.
    Old(&'a [Node]),
    /// Прежнего нет: всё новое.
    Fresh,
}

pub(super) struct Level<'a> {
    pub(super) nodes: &'a [Node],
    pub(super) at: usize,
    pub(super) parent: BoxId,
    pub(super) owner: Option<&'a Element>,
    pub(super) prev: Prev<'a>,
    last: BoxId,
    /// Следующая прежняя коробка-ребёнок по порядку.
    cursor: BoxId,
    /// Прежние дети после первого расхождения порядка.
    rest: Option<Reuse>,
    reshaped: bool,
}

impl<'a> Level<'a> {
    pub(super) fn new(
        nodes: &'a [Node],
        parent: BoxId,
        owner: Option<&'a Element>,
        prev: Prev<'a>,
        old_first: BoxId,
    ) -> Self {
        Self {
            nodes,
            at: 0,
            parent,
            owner,
            prev,
            last: BoxId::NONE,
            cursor: old_first,
            rest: None,
            reshaped: false,
        }
    }

    pub(super) fn next_node(&mut self) -> Option<&'a Node> {
        let node = self.nodes.get(self.at)?;
        self.at += 1;
        Some(node)
    }

    /// Прежняя коробка этого источника среди детей того же родителя: сначала
    /// по порядку, после расхождения — по источнику.
    pub(super) fn take(&mut self, tree: &BoxTree, origin: &Source) -> Option<BoxId> {
        if self.rest.is_none() {
            if let Some(record) = tree.get(self.cursor)
                && record.origin == *origin
            {
                let id = self.cursor;
                self.cursor = record.next;
                return Some(id);
            }
            let mut rest = Reuse::default();
            while let Some(record) = tree.get(self.cursor) {
                rest.insert(record.origin.clone(), self.cursor);
                self.cursor = record.next;
            }
            self.rest = Some(rest);
        }
        self.reshaped = true;
        self.rest.as_mut()?.take(origin)
    }

    pub(super) fn append(&mut self, tree: &mut BoxTree, id: BoxId) {
        if self.last.is_some() {
            if let Some(last) = tree.arena.record_mut(self.last) {
                last.next = id;
            }
        } else if self.parent.is_some() {
            if let Some(parent) = tree.arena.record_mut(self.parent) {
                parent.first = id;
            }
        } else {
            tree.first = id;
        }
        self.last = id;
    }

    /// Убрать прежних детей, не нашедших источника; `true`, если список детей
    /// изменился.
    pub(super) fn finish(self, tree: &mut BoxTree) -> bool {
        let mut reshaped = self.reshaped;
        let mut cursor = self.cursor;
        while let Some(next) = tree.get(cursor).map(|r| r.next) {
            tree.remove_subtree(cursor);
            cursor = next;
            reshaped = true;
        }
        for id in self.rest.into_iter().flat_map(Reuse::into_ids) {
            tree.remove_subtree(id);
            reshaped = true;
        }
        reshaped
    }
}

/// Прежний элемент на месте `at` среди `new` братьев: тот же индекс либо
/// сдвиг на разницу длин (одна вставка или удаление перед ним).
pub(super) fn paired(old: &[Node], new: usize, at: usize, key: u64) -> Option<&Element> {
    let shifted = (at + old.len()).checked_sub(new);
    [Some(at), shifted]
        .into_iter()
        .flatten()
        .find_map(|i| match old.get(i) {
            Some(Node::Element(e)) if e.node_id == key => Some(e),
            _ => None,
        })
}

pub(super) fn texts_differ(old: &[Node], new: &[Node]) -> bool {
    !texts(old).eq(texts(new))
}

fn texts(nodes: &[Node]) -> impl Iterator<Item = &str> + '_ {
    nodes.iter().filter_map(|n| match n {
        Node::Text(t) => Some(t.as_str()),
        Node::Element(_) => None,
    })
}
