//! Сборка кадра внутри `Window::draw`: запись не подошла, а шаг документа
//! дерево не подготовил, подготовил для другого места в кадре, или
//! подготовленное отказала проверка заместителей.

use super::Retained;
use gpui::{App, AvailableSpace, Bounds, ElementPlace, Pixels, RetainedPrepaint, Window, size};

impl Retained {
    /// Место документа в этом кадре. Дерево шага, собранное для другого
    /// места, не годится: ключи состояний его элементов отличались бы от
    /// кадра — тогда кадр собирается здесь, как без шага.
    pub(super) fn adopt_place(&mut self, place: ElementPlace) {
        if self.laid_out.as_ref().is_some_and(|laid| *laid != place) {
            self.laid_out = None;
            self.scheduled = 0;
            self.slot.builds.set(self.slot.builds.get() + 1);
            self.slot.inside_draw.set(self.slot.inside_draw.get() + 1);
            let (tree, flow) = self.builder.build(true, false);
            self.tree = Some(tree);
            self.flow = flow;
        }
        self.slot.note_place(place);
    }

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
