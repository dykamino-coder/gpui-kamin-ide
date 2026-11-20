//! Draw-time layout for a host that did not prepare this placement.

use super::Retained;
use crate::document::retained::UpdatePhase;
use gpui::{App, Bounds, LayoutId, Pixels, RetainedPrepaint, Style, Window, px, size};

impl Retained {
    pub(super) fn request_viewport_layout(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> LayoutId {
        self.check_place(window);
        let before = window.next_frame_callback_count();
        let id = if self.laid_out.is_none()
            && let Some(tree) = self.tree.as_mut()
        {
            let keys = self.builder.layout_keys;
            let (id, inputs) = window.record_layout_inputs(|window| {
                window.with_layout_keys(keys, |window| tree.request_layout(window, cx))
            });
            self.layout_inputs = inputs;
            id
        } else {
            let (w, h) = self.builder.opts.viewport;
            let style = Style {
                size: size(px(w).into(), px(h).into()),
                ..Style::default()
            };
            window.request_layout(style, None, cx)
        };
        self.scheduled += window.next_frame_callback_count().saturating_sub(before);
        id
    }

    /// Дерево шага, собранное для другого места в кадре, не годится: ключи
    /// состояний его элементов отличались бы от кадра — тогда кадр
    /// собирается здесь, как без шага.
    fn check_place(&mut self, window: &Window) {
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
        let (mut tree, flow, inputs) = self.builder.layout_checked(bounds, window, cx, |phase| {
            if phase == UpdatePhase::Built {
                self.slot.builds.set(self.slot.builds.get() + 1);
                self.slot.inside_draw.set(self.slot.inside_draw.get() + 1);
            }
        });
        self.layout_inputs = inputs;
        let (_, prepaint) =
            window.record_retained_prepaint(|w| tree.prepaint_at(bounds.origin, w, cx));
        self.tree = Some(tree);
        self.flow = flow;
        prepaint
    }
}
