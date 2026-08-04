//! Верх файловой колонки: редактор с табами.
//!
//! Ветка `top_content` вынесена как есть (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Window, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn file_top_editor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        p: &'static Palette,
        viewport_h: f32,
    ) -> AnyElement {
        // РЕДАКТОР: полоса файл-табов + активный code_editor + Save
        let tabs_meta: Vec<(String, bool, bool)> = self
            .ed
            .editor_tabs
            .iter()
            .map(|t| (t.path.clone(), t.dirty, t.pinned))
            .collect();
        // Ширины табов меряем шейпером — переполнение стрипа должно
        // считаться по реальному тексту, а не по «символы × 6.5»
        let tab_widths: Vec<f32> = tabs_meta
            .iter()
            .map(|(path, dirty, pinned)| {
                crate::ui::editor_tabs::tab_width(
                    &crate::ui::editor_tabs::base_name(path),
                    *pinned,
                    *dirty,
                    window,
                )
            })
            .collect();
        let active = self.ed.editor_active.min(self.ed.editor_tabs.len() - 1);
        let input = &self.ed.editor_tabs[active].input;
        let dirty = self.ed.editor_tabs[active].dirty;
        div()
            .flex()
            .flex_col()
            // `.viewer { flex: 1 }` — карта ДЕЛИТ место колонки, а не
            // берёт её ширину целиком: при `size_full` поля `mx(6)`
            // прибавлялись СНАРУЖИ 100 %, и карта вылезала на 12 px
            // вправо, в правую панель (баг найден юзером; ревью ц.25
            // отметило `flex: 1` против `size_full` отдельно)
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            // `.viewer { margin: 0 6px 6px; background: bg-mantle;
            // border-radius: radius-md; overflow: hidden }` — карта
            // держит И таб-стрип, И тело; раньше инсеты висели на
            // теле, и стрип оказывался ВНЕ рамки (ревью ц.23)
            .mx(px(6.0))
            .mb(px(6.0))
            // Досье 108 — КАРТА целиком; `file-tabs` это её полоса
            // табов (досье 110), общий регион давал один кроп
            .child(crate::probe::registry::probe_area("file-viewer-wrapper"))
            .bg(rgba(p.bg_mantle))
            .rounded(px(m::RADIUS_MD))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w(px(0.))
                            .child(crate::probe::registry::probe_area("file-tabs"))
                            .child(crate::ui::editor_tabs::editor_tabs_bar(
                                &tabs_meta,
                                &tab_widths,
                                active,
                                self.ed
                                    .tab_drag
                                    .as_ref()
                                    .filter(|d| d.started)
                                    .and_then(|d| d.over),
                                self.ed
                                    .tab_drag
                                    .as_ref()
                                    .filter(|d| d.started)
                                    .map(|d| d.src),
                                crate::probe::registry::bounds_of("file-tabs")
                                    .map(|[_, _, w, _]| w - 16.0)
                                    .unwrap_or(f32::MAX),
                                viewport_h,
                                self.ed.file_tabs_overflow_open,
                                &self.ed.tabs_scroll,
                                std::mem::take(&mut self.ed.tabs_reveal_active),
                                &self.tx,
                                p,
                            )),
                    )
                    .when(dirty, |d| {
                        d.child(
                            div()
                                .id("editor-save")
                                .flex_shrink_0()
                                .mx(px(m::SPACE_2))
                                .px(px(m::SPACE_3))
                                .py(px(3.0))
                                .rounded(px(m::RADIUS_SM))
                                .bg(rgba(p.accent_action))
                                .text_size(px(m::FS_XS))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgba(p.accent_action_fg))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.9))
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    cx.listener(|this, _, window, cx| {
                                        this.save_editor(window, cx);
                                    }),
                                )
                                .child("Save  Ctrl+S"),
                        )
                    }),
            )
            .child({
                let apath = self.ed.editor_tabs[active].path.clone();
                let input = input.clone();
                self.file_editor_body(&input, apath, cx, p)
            })
            .into_any_element()
    }
}
