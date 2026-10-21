//! Typed-tree arena insertion and attribute storage.
use super::*;
impl<'input> Document<'input> {
    // KaminIDE patch: every fallible parser insertion checks the arena budget.
    pub(crate) fn check_node_budget(&self) -> Result<(), Error> {
        if self.nodes.len() >= self.node_limit {
            Err(Error::NodesLimitReached)
        } else {
            Ok(())
        }
    }

    pub(crate) fn append(&mut self, parent_id: NodeId, kind: NodeKind) -> NodeId {
        let new_child_id = NodeId::from(self.nodes.len());
        self.nodes.push(NodeData {
            parent: Some(parent_id),
            next_sibling: None,
            children: None,
            kind,
        });

        let last_child_id = self.nodes[parent_id.get_usize()].children.map(|(_, id)| id);

        if let Some(id) = last_child_id {
            self.nodes[id.get_usize()].next_sibling = Some(new_child_id);
        }

        self.nodes[parent_id.get_usize()].children = Some(
            if let Some((first_child_id, _)) = self.nodes[parent_id.get_usize()].children {
                (first_child_id, new_child_id)
            } else {
                (new_child_id, new_child_id)
            },
        );

        new_child_id
    }

    pub(super) fn append_attribute(
        &mut self,
        name: AId,
        value: roxmltree::StringStorage<'input>,
        important: bool,
    ) {
        self.attrs.push(Attribute {
            name,
            value,
            important,
        });
    }
}
