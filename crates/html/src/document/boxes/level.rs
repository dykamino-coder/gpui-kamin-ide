//! One level of the publication walk: new siblings, their previous boxes and the comparison
//! with the previous input.

use super::BoxTree;
use super::claims::Claims;
use super::reuse::Reuse;
use super::source::Source;
use crate::dom::{BoxId, Element, Node};

/// What the siblings of a walk level are compared with.
#[derive(Clone, Copy)]
pub(super) enum Prev<'a> {
    /// A snapshot shared with the previous input: the subtree did not change.
    Clean,
    /// The previous siblings of the same parent.
    Old(&'a [Node]),
    /// There is no previous input: everything is new.
    Fresh,
}

pub(super) struct Level<'a> {
    pub(super) nodes: &'a [Node],
    pub(super) at: usize,
    pub(super) parent: BoxId,
    pub(super) owner: Option<&'a Element>,
    pub(super) prev: Prev<'a>,
    /// The ancestors of the level passed `epochs::same_layout_inputs`: the parent has the same
    /// computed styles.
    pub(super) inherited: bool,
    last: BoxId,
    /// The next previous child box in order.
    cursor: BoxId,
    /// The previous children after the first divergence of the order.
    rest: Option<Reuse>,
    reshaped: bool,
}

impl<'a> Level<'a> {
    pub(super) fn new(
        nodes: &'a [Node],
        parent: BoxId,
        owner: Option<&'a Element>,
        prev: Prev<'a>,
        inherited: bool,
        old_first: BoxId,
    ) -> Self {
        Self {
            nodes,
            at: 0,
            parent,
            owner,
            prev,
            inherited,
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

    /// The previous box of this source among the children of the same parent: first
    /// in order, after a divergence by source.
    pub(super) fn take(
        &mut self,
        tree: &BoxTree,
        claims: &mut Claims,
        origin: &Source,
    ) -> Option<BoxId> {
        if self.rest.is_none() {
            while claims.used(self.cursor) {
                self.cursor = claims.next(tree, self.cursor)?;
                self.reshaped = true;
            }
            if let Some(record) = tree.get(self.cursor)
                && record.origin == *origin
            {
                let id = self.cursor;
                self.cursor = record.next;
                claims.accept(id);
                return Some(id);
            }
            let mut rest = Reuse::default();
            while let Some(record) = tree.get(self.cursor) {
                if !claims.used(self.cursor) {
                    rest.insert(record.origin.clone(), self.cursor);
                }
                self.cursor = claims.next(tree, self.cursor).unwrap_or(BoxId::NONE);
            }
            self.rest = Some(rest);
        }
        self.reshaped = true;
        while let Some(id) = self.rest.as_mut()?.take(origin) {
            if !claims.used(id) {
                claims.accept(id);
                return Some(id);
            }
        }
        None
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

    /// Remove the previous children that found no source; `true` if the child list
    /// changed.
    pub(super) fn finish(self, tree: &BoxTree, claims: &mut Claims) -> bool {
        let mut reshaped = self.reshaped;
        let mut cursor = self.cursor;
        while let Some(next) = claims.next(tree, cursor) {
            if !claims.used(cursor) {
                claims.remove(cursor);
            }
            cursor = next;
            reshaped = true;
        }
        for id in self.rest.into_iter().flat_map(Reuse::into_ids) {
            if !claims.used(id) {
                claims.remove(id);
            }
            reshaped = true;
        }
        reshaped
    }
}

/// The previous element at `at` among `new` siblings: the same index or
/// shifted by the length difference (one insertion or removal before it).
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
