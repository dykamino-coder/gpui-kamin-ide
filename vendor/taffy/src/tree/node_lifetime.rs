//! Node teardown releases measurement contexts alongside native tree state.

use super::{TaffyResult, TaffyTree};
use crate::NodeId;

impl<NodeContext> TaffyTree<NodeContext> {
    /// Drops all nodes in the tree
    pub fn clear(&mut self) {
        self.node_context_data.clear();
        self.nodes.clear();
        self.children.clear();
        self.parents.clear();
    }

    /// Remove a specific node from the tree and drop it
    ///
    /// Returns the id of the node removed.
    pub fn remove(&mut self, node: NodeId) -> TaffyResult<NodeId> {
        let key = node.into();
        if let Some(parent) = self.parents[key] {
            if let Some(children) = self.children.get_mut(parent.into()) {
                children.retain(|f| *f != node);
            }
            self.mark_dirty(parent)?;
        }

        // Remove "parent" references to a node when removing that node
        if let Some(children) = self.children.get(key) {
            for child in children.iter().copied() {
                self.parents[child.into()] = None;
            }
        }

        let _ = self.children.remove(key);
        let _ = self.parents.remove(key);
        let _ = self.node_context_data.remove(key);
        let _ = self.nodes.remove(key);

        Ok(node)
    }
}

#[cfg(test)]
#[path = "node_lifetime_tests.rs"]
mod tests;
