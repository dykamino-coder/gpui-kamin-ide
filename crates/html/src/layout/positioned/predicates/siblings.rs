//! "Is there such a node later in the list" for every node of one built list
//! in O(n): suffix answers, made on the first question and kept for the
//! build of this list.
//!
//! The list is the one the flow builds from (after anonymous wrappers, float
//! wrapping and paint reordering), so the answers cannot live in the DOM
//! children snapshot; the per-subtree part of each answer does
//! (`Children::positioned_fact`), and a suffix costs one look per node.
//! A new build of the list (DOM mutation, style change) builds a new
//! `Siblings`, so the answers never outlive their nodes.

use super::{later_node, stays_node};
use crate::dom::{Element, Node};
use std::cell::OnceCell;

/// One built list of siblings with lazily made suffix answers.
pub(crate) struct Siblings<'a> {
    nodes: &'a [Node],
    stays: OnceCell<Box<[bool]>>,
    later: OnceCell<Box<[bool]>>,
}

impl<'a> Siblings<'a> {
    pub(crate) fn new(nodes: &'a [Node]) -> Self {
        Self {
            nodes,
            stays: OnceCell::new(),
            later: OnceCell::new(),
        }
    }

    /// The nodes after `idx`.
    pub(crate) fn after(&self, idx: usize) -> Later<'_> {
        Later { list: self, idx }
    }

    fn suffix(
        &self,
        cell: &OnceCell<Box<[bool]>>,
        node: fn(&Element) -> bool,
        from: usize,
    ) -> bool {
        let answers = cell.get_or_init(|| {
            let mut answers = vec![false; self.nodes.len()].into_boxed_slice();
            let mut any = false;
            for (i, n) in self.nodes.iter().enumerate().rev() {
                any = any || matches!(n, Node::Element(e) if node(e));
                answers[i] = any;
            }
            answers
        });
        answers.get(from).copied().unwrap_or(false)
    }
}

impl std::ops::Deref for Siblings<'_> {
    type Target = [Node];
    fn deref(&self) -> &[Node] {
        self.nodes
    }
}

/// The nodes after one node of a `Siblings` list.
#[derive(Clone, Copy)]
pub(crate) struct Later<'a> {
    list: &'a Siblings<'a>,
    idx: usize,
}

impl<'a> Later<'a> {
    /// Stays a positioned node after this one in place (`stays_node`)?
    pub(crate) fn stays_positioned(self) -> bool {
        self.list
            .suffix(&self.list.stays, |e| stays_node(e, false), self.idx + 1)
    }

    /// Paints positioned content after this one in the first pass (`later_node`)?
    pub(crate) fn positioned_later(self) -> bool {
        self.list.suffix(&self.list.later, later_node, self.idx + 1)
    }
}

#[cfg(test)]
mod tests;
