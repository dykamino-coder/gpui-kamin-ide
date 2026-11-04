//! Продолжение безопасного выравнивания удерживает сдвиг до завершения prepaint.

use super::*;

pub(super) struct SafePlace {
    pub(super) child: AnyElement,
    pub(super) plan: Plan,
    /// Values of the document the plan's lengths come from.
    pub(super) values: crate::values::ValueDoc,
    /// Layout state of the same document: containing-block registry.
    pub(super) layout: std::rc::Rc<crate::layout::layout_state::LayoutState>,
    pub(super) build: ChildBuild,
}
impl Wrap for SafePlace {
    fn child(&mut self) -> &mut AnyElement {
        &mut self.child
    }

    async fn prepaint_async(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        let shift = self.plan.shift(
            bounds,
            window,
            self.values.vcx(),
            &self.layout.anchors,
            self.build,
        );
        window
            .with_exact_element_offset_async(shift, async |window| {
                self.child.prepaint_async(window, cx, stack).await
            })
            .await;
    }
}
