//! Состав детей и перемещение публикуются раздельно: перестановка сохраняет родителя.

use super::Changes;
use crate::document::delta::{SourceChange, SourceChangeKind, SourceDelta};
use crate::dom::NodeKey;
use std::collections::HashMap;

type Parents = HashMap<NodeKey, (Option<NodeKey>, Option<NodeKey>)>;

impl Changes {
    pub(in crate::document::revision::mutation) fn changed_parents(&self) -> Parents {
        self.links
            .iter()
            .map(|(node, old, new)| {
                let key = |weak: &Option<markup5ever_rcdom::WeakHandle>| {
                    weak.as_ref()
                        .and_then(|weak| weak.upgrade())
                        .map(|node| node.key())
                };
                (node.key(), (key(old), key(new)))
            })
            .collect()
    }

    pub(in crate::document::revision::mutation) fn append_delta(
        &self,
        delta: &mut SourceDelta,
        parents: &Parents,
    ) {
        for (node, _) in &self.lists {
            let (before_parent, after_parent) =
                parents.get(&node.key()).copied().unwrap_or_else(|| {
                    let parent = super::parent(node).map(|parent| parent.key());
                    (parent, parent)
                });
            delta.changes.push(SourceChange {
                node: node.key(),
                kind: SourceChangeKind::Children,
                before_parent,
                after_parent,
            });
        }
        for &(ref node, _, _) in &self.links {
            let &(before_parent, after_parent) = parents.get(&node.key()).expect("changed parent");
            delta.changes.push(SourceChange {
                node: node.key(),
                kind: SourceChangeKind::Reparent,
                before_parent,
                after_parent,
            });
        }
    }
}
