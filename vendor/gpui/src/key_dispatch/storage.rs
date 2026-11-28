//! KaminIDE patch: буферы дерева ввода следуют живому кадру, включая replay.

use super::{DispatchNodeId, DispatchTree, EntityId, FocusId};

impl DispatchTree {
    pub(super) fn release_retired_storage(&mut self) {
        // Сжимаем до очистки: неизменный кадр сохраняет буфер, меньший
        // освобождает место прежнего документа, пустой — весь буфер.
        self.nodes.shrink_to_fit();
        self.focusable_node_ids.shrink_to_fit();
        self.view_node_ids.shrink_to_fit();
        if self.nodes.is_empty() {
            self.node_stack.shrink_to_fit();
            self.context_stack.shrink_to_fit();
            self.view_stack.shrink_to_fit();
        }
    }

    pub(crate) fn heap_report(&self, sink: &mut crate::heap_report::HeapSink) {
        use crate::heap_report::{boxed, table, vec};
        let path = "gpui/dispatch-tree";
        vec(sink, path, &self.nodes);
        vec(sink, path, &self.node_stack);
        vec(sink, path, &self.context_stack);
        vec(sink, path, &self.view_stack);
        table::<(FocusId, DispatchNodeId)>(sink, path, self.focusable_node_ids.capacity());
        table::<(EntityId, DispatchNodeId)>(sink, path, self.view_node_ids.capacity());
        for node in &self.nodes {
            if let Some(rare) = &node.rare {
                boxed(sink, path, &**rare);
                vec(sink, path, &rare.key_listeners);
                vec(sink, path, &rare.action_listeners);
                vec(sink, path, &rare.modifiers_changed_listeners);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ActionRegistry, KeyContext, Keymap};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    fn test_dispatch_tree() -> DispatchTree {
        DispatchTree::new(
            Rc::new(RefCell::new(Keymap::new(Vec::new()))),
            Rc::new(ActionRegistry::default()),
        )
    }

    fn fill(tree: &mut DispatchTree, count: usize) {
        for _ in 0..count {
            tree.push_node();
            tree.pop_node();
        }
    }

    #[test]
    fn a_small_frame_releases_retired_nodes_and_empty_releases_every_buffer() {
        let mut tree = test_dispatch_tree();
        fill(&mut tree, 4096);
        tree.clear();
        let large = tree.nodes.capacity();
        fill(&mut tree, 4096);
        tree.clear();
        assert_eq!(tree.nodes.capacity(), large);
        fill(&mut tree, 2);
        tree.clear();
        let small = tree.nodes.capacity();
        assert!(small >= 2 && small < large / 16, "{large} -> {small}");
        for _ in 0..3 {
            fill(&mut tree, 2);
            tree.clear();
            assert_eq!(tree.nodes.capacity(), small);
        }
        tree.clear();
        assert_eq!(tree.nodes.capacity(), 0);
        assert_eq!(tree.node_stack.capacity(), 0);
        assert_eq!(tree.context_stack.capacity(), 0);
        assert_eq!(tree.view_stack.capacity(), 0);
        assert_eq!(tree.focusable_node_ids.capacity(), 0);
        assert_eq!(tree.view_node_ids.capacity(), 0);
    }

    #[test]
    fn replay_keeps_context_and_listener_until_the_last_frame_retires() {
        let mut old = test_dispatch_tree();
        let retained = Rc::new(Cell::new(1));
        old.push_node();
        old.set_key_context(KeyContext::parse("Editor").unwrap());
        let listener_value = retained.clone();
        old.on_key_event(Rc::new(move |_, _, _, _| {
            listener_value.set(2);
        }));
        old.pop_node();
        let mut next = test_dispatch_tree();
        next.reuse_subtree(0..1, &mut old, None);
        old.clear();
        old.clear();
        assert!(next.node(DispatchNodeId(0)).context().is_some());
        assert_eq!(next.node(DispatchNodeId(0)).key_listeners().len(), 1);
        assert_eq!(Rc::strong_count(&retained), 2);
        let mut bytes = 0;
        next.heap_report(&mut |owner, _, size| {
            assert_eq!(owner, "gpui/dispatch-tree");
            bytes += size;
        });
        assert!(bytes >= std::mem::size_of::<super::super::DispatchNode>());
        next.clear();
        assert_eq!(Rc::strong_count(&retained), 1);
        next.clear();
        let mut empty_bytes = 0;
        next.heap_report(&mut |_, _, size| empty_bytes += size);
        assert_eq!(empty_bytes, 0);
    }
}
