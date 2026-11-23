//! Moves retain a box until both ends of its parent link are published.
//!
//! The old parent may be visited first, so unclaimed boxes retire after traversal.
//! A moved box retains its old next link until publication ends because its new
//! siblings may already have overwritten its links.

use super::{BoxTree, source::Source};
use crate::document::delta::BoxDelta;
use crate::dom::BoxId;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct Claims {
    used: HashSet<BoxId>,
    kept: HashSet<BoxId>,
    reserved: HashSet<BoxId>,
    moved: HashMap<BoxId, BoxId>,
    removed: Vec<BoxId>,
    active: bool,
}

impl Claims {
    pub(super) fn new(tree: &BoxTree) -> Self {
        Self {
            active: !tree.is_empty(),
            ..Self::default()
        }
    }

    pub(super) fn used(&self, id: BoxId) -> bool {
        self.used.contains(&id)
    }

    pub(super) fn accept(&mut self, id: BoxId) {
        if self.active {
            self.used.insert(id);
        }
    }

    pub(super) fn reserve(&mut self, id: BoxId) {
        self.reserved.insert(id);
    }

    pub(super) fn keep(&mut self, id: BoxId) {
        if self.active {
            self.kept.insert(id);
        }
    }

    pub(super) fn next(&self, tree: &BoxTree, id: BoxId) -> Option<BoxId> {
        self.moved
            .get(&id)
            .copied()
            .or_else(|| tree.get(id).map(|r| r.next))
    }

    pub(super) fn take(&mut self, tree: &BoxTree, origin: &Source, source: u64) -> Option<BoxId> {
        if !self.active || !matches!(origin, Source::Node(_)) {
            return None;
        }
        let id = tree.by_node.all(source).find(|&id| {
            !self.used(id)
                && !self.reserved.contains(&id)
                && tree.get(id).is_some_and(|r| r.origin == *origin)
                && !tree.ancestors(id).any(|parent| self.kept.contains(&parent))
        })?;
        self.moved.insert(id, tree.get(id)?.next);
        self.accept(id);
        Some(id)
    }

    pub(super) fn remove(&mut self, id: BoxId) {
        if id.is_some() {
            self.removed.push(id);
        }
    }

    pub(super) fn finish(mut self, tree: &mut BoxTree, delta: &mut BoxDelta) {
        let mut pending = std::mem::take(&mut self.removed);
        while let Some(id) = pending.pop() {
            if self.moved.contains_key(&id) {
                continue;
            }
            let Some(record) = tree.get(id) else { continue };
            let mut child = record.first;
            while let Some(next) = self.next(tree, child) {
                pending.push(child);
                child = next;
            }
            // Retire children separately: a child may already have been claimed.
            tree.arena.unlink_first(id);
            tree.remove_subtree(id, delta);
        }
    }
}
