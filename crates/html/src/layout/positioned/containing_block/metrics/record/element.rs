//! Обёртка исходного узла связывает normal-геометрию и собственный допуск paint.

use super::*;

impl Recording {
    pub(crate) fn wrap(self: Box<Self>, child: AnyElement) -> AnyElement {
        Record {
            geometry: self,
            child,
            node: None,
        }
        .hooked()
    }
}

struct Record {
    geometry: Box<Recording>,
    child: AnyElement,
    node: Option<Node>,
}

impl Wrap for Record {
    fn paint_boundary(&self) -> bool {
        false
    }
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> LayoutId {
        let id = self.child.request_layout(window, cx);
        let node = Node::capture(id, window);
        self.node = Some(node);
        self.geometry.remember(node);
        id
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        // A settling pass keeps the instance prepaint placed: of the candidate
        // boxes `position-try` lays out, only the chosen one is prepainted.
        if let Some(node) = self.node
            && window
                .render_context::<super::Settling>()
                .is_some_and(|s| Rc::ptr_eq(&s.0, &self.geometry.recorder))
        {
            self.geometry.remember(node);
            super::settle::note(
                &self.geometry.recorder.records,
                self.geometry.id,
                bounds.origin,
            );
        }
        let geometry = &self.geometry;
        crate::hooks::paint_source_node(
            geometry.id,
            geometry.positioned.then_some(geometry.cb),
            bounds.origin,
            window,
            cx,
            |window, cx| {
                self.child.prepaint(window, cx);
            },
        );
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let geometry = &self.geometry;
        crate::hooks::paint_source_node(
            geometry.id,
            geometry.positioned.then_some(geometry.cb),
            bounds.origin,
            window,
            cx,
            |window, cx| self.child.paint(window, cx),
        );
    }
}

impl Recording {
    fn remember(&self, node: Node) {
        self.recorder.nodes.borrow_mut().insert(
            self.id,
            Entry {
                node,
                cb: self.cb,
                positioned: self.positioned,
                border: self.border,
                clone_edges: self.clone_edges,
            },
        );
    }
}
