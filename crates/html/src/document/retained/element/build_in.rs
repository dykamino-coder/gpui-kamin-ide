//! Draw-time layout for a host that did not prepare this placement.

use super::Retained;
use crate::document::retained::UpdatePhase;
use gpui::{App, Bounds, Pixels, RetainedPrepaint, Window};

impl Retained {
    /// Дерево шага, собранное для другого места в кадре, не годится: ключи
    /// состояний его элементов отличались бы от кадра — тогда кадр
    /// собирается здесь, как без шага.
    pub(super) fn check_place(&mut self, window: &Window) {
        if self
            .laid_out
            .as_ref()
            .is_some_and(|parent| !window.element_stands_under(parent))
        {
            self.laid_out = None;
            self.checked = None;
            self.scheduled = 0;
            self.slot.builds.set(self.slot.builds.get() + 1);
            self.slot.inside_draw.set(self.slot.inside_draw.get() + 1);
            let (tree, flow) = self.builder.build(true, false);
            self.tree = Some(tree);
            self.flow = flow;
        }
    }

    /// Дерево не подошло к записи: собрать и разложить в тех же границах.
    pub(super) fn build_in(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> RetainedPrepaint {
        let (mut tree, flow) = self.builder.layout_checked(bounds, window, cx, |phase| {
            if phase == UpdatePhase::Built {
                self.slot.builds.set(self.slot.builds.get() + 1);
                self.slot.inside_draw.set(self.slot.inside_draw.get() + 1);
            }
        });
        let (_, prepaint) =
            window.record_retained_prepaint(|w| tree.prepaint_at(bounds.origin, w, cx));
        self.tree = Some(tree);
        self.flow = flow;
        prepaint
    }
}
