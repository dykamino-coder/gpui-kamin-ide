//! Mutation roots reach both source ancestry and retained output containment.
use super::{NodeKey, SourceMemo};
use crate::dom::source_delta::{SourceChangeKind, SourceDelta};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct DirtyEntries {
    direct: HashSet<NodeKey>,
    paths: HashSet<NodeKey>,
}

impl DirtyEntries {
    pub(super) fn new(memo: &SourceMemo, delta: &SourceDelta) -> Self {
        let mut dirty = Self::default();
        let mut parents = HashMap::new();
        for change in &delta.changes {
            parents.insert(change.node, change.after_parent);
        }
        for change in &delta.changes {
            if !matches!(change.kind, SourceChangeKind::CharacterData) {
                dirty.direct.insert(change.node);
            }
            // A Children record names the changed sequence itself; its parent
            // link is ancestry, not another changed sequence.
            let sequences = match change.kind {
                SourceChangeKind::Attributes(_) => [None, None],
                SourceChangeKind::Children => [Some(change.node), None],
                SourceChangeKind::CharacterData | SourceChangeKind::Reparent => {
                    [change.before_parent, change.after_parent]
                }
            };
            for sequence in sequences.into_iter().flatten() {
                dirty.sequence(memo, sequence);
            }
        }
        for node in &dirty.direct {
            Self::path(memo, *node, &parents, &mut dirty.paths);
        }
        dirty
    }

    fn sequence(&mut self, memo: &SourceMemo, sequence: NodeKey) {
        // Sibling position and :empty can change ancestor matching for every
        // descendant. Generated inputs alone cannot prove these entries reusable.
        let mut work = vec![sequence];
        let mut visited = HashSet::new();
        if memo.output().ownership.sequence == Some(sequence) {
            work.extend_from_slice(&memo.output().ownership.children);
        }
        work.extend_from_slice(memo.readers(sequence));
        while let Some(node) = work.pop() {
            if !visited.insert(node) {
                continue;
            }
            self.direct.insert(node);
            if let Some(entry) = memo.get(node) {
                work.extend_from_slice(&entry.ownership.children);
            }
            work.extend_from_slice(memo.readers(node));
        }
    }

    fn path(
        memo: &SourceMemo,
        node: NodeKey,
        parents: &HashMap<NodeKey, Option<NodeKey>>,
        paths: &mut HashSet<NodeKey>,
    ) {
        let mut work = vec![node];
        while let Some(node) = work.pop() {
            if !paths.insert(node) {
                continue;
            }
            work.extend(parents.get(&node).copied().flatten());
            if let Some(entry) = memo.get(node) {
                work.extend(entry.source_parent);
                work.extend(entry.ownership.parent);
            }
            // Shadow roots have no element entry. Slots can read their host's
            // light sequence even though the matching spine skips the slot.
            work.extend_from_slice(memo.readers(node));
        }
    }

    pub fn contains(&self, node: NodeKey) -> bool {
        self.paths.contains(&node)
    }

    pub fn directly_changes(&self, node: NodeKey) -> bool {
        self.direct.contains(&node)
    }

    /// Selector and generated-content invalidation add dependency roots here.
    pub fn invalidate(&mut self, memo: &SourceMemo, node: NodeKey) {
        self.direct.insert(node);
        Self::path(memo, node, &HashMap::new(), &mut self.paths);
    }
}
