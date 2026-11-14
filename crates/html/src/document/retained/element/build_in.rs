//! Сборка кадра внутри prepaint: запись не подошла, а шаг документа дерево не
//! подготовил (или подготовленное отказала проверка заместителей).

use super::Retained;
use gpui::{App, AvailableSpace, Bounds, Pixels, RetainedPrepaint, Window, size};

impl Retained {
    /// Дерево не подошло к записи: собрать и разложить в тех же границах.
    pub(super) fn build_in(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> RetainedPrepaint {
        let space = size(
            AvailableSpace::Definite(bounds.size.width),
            AvailableSpace::Definite(bounds.size.height),
        );
        let (_, prepaint) = window.record_retained_prepaint(|window| {
            // Заместители, отказанные проверкой прошлой сборки, собираются;
            // если и эта не прошла (другое место корня), то без заместителей.
            let mut allow = true;
            loop {
                self.slot.builds.set(self.slot.builds.get() + 1);
                self.slot.inside_draw.set(self.slot.inside_draw.get() + 1);
                let (mut tree, flow) = self.builder.build(allow, true);
                tree.layout_as_root(space, window, cx);
                let fits = flow.as_ref().is_none_or(|flow| flow.validate(window));
                if fits || !allow {
                    tree.prepaint_at(bounds.origin, window, cx);
                    self.tree = Some(tree);
                    self.flow = flow;
                    return;
                }
                if let Some(flow) = &flow {
                    flow.note_rebuild();
                }
                allow = false;
            }
        });
        prepaint
    }
}
