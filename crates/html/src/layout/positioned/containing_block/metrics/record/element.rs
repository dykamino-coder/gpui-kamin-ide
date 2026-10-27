//! Обёртка исходного узла связывает normal-геометрию и собственный допуск paint.

use super::*;

impl Recording {
    pub(crate) fn wrap(self: Box<Self>, child: AnyElement) -> AnyElement {
        Record {
            geometry: self,
            child,
        }
        .hooked()
    }
}

struct Record {
    geometry: Box<Recording>,
    child: AnyElement,
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
        let geometry = &self.geometry;
        geometry.recorder.nodes.borrow_mut().insert(
            geometry.id,
            Entry {
                node: Node::capture(id, window),
                cb: geometry.cb,
                positioned: geometry.positioned,
                border: geometry.border,
                clone_edges: geometry.clone_edges,
            },
        );
        id
    }

    fn prepaint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let geometry = &self.geometry;
        crate::hooks::paint_source_node(
            geometry.id,
            geometry.positioned.then_some(geometry.cb),
            window,
            |window| {
                self.child.prepaint(window, cx);
            },
        );
    }

    fn paint(&mut self, _: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let geometry = &self.geometry;
        crate::hooks::paint_source_slot(geometry.id, window, cx);
        crate::hooks::paint_source_node(
            geometry.id,
            geometry.positioned.then_some(geometry.cb),
            window,
            |window| self.child.paint(window, cx),
        );
    }
}
