//! File opening, folder selection, viewport presets, and window controls.

use crate::{app::Browser, viewport};
use gpui::{Context, IntoElement, Window, div, prelude::*, px, rgb};

pub fn button(
    label: impl Into<gpui::SharedString>,
    id: impl Into<gpui::ElementId>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_shrink_0()
        .px_2()
        .py_1()
        .rounded_sm()
        .bg(rgb(0x364152))
        .hover(|style| style.bg(rgb(0x4b5b70)))
        .cursor_pointer()
        .child(label.into())
}

impl Browser {
    pub fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let path = self
            .current
            .as_ref()
            .or(self.folder.as_ref())
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "No page selected".into());
        let open = div()
            .id("open-toolbar")
            .flex()
            .items_center()
            .gap_2()
            .h(px(38.))
            .overflow_x_scroll()
            .child(
                button("Open file", "open-file")
                    .on_click(cx.listener(|this, _, window, cx| this.prompt(false, window, cx))),
            )
            .child(
                button("Open folder", "open-folder")
                    .on_click(cx.listener(|this, _, window, cx| this.prompt(true, window, cx))),
            )
            .child(
                button(
                    if window.is_maximized() || window.is_fullscreen() {
                        "Restore"
                    } else {
                        "Maximize"
                    },
                    "maximize",
                )
                .on_click(cx.listener(|this, _, window, _| {
                    this.preset_width = None;
                    crate::window::maximize_or_restore(window);
                })),
            )
            .child(div().flex_shrink_0().child(path));
        let mut controls = div()
            .id("view-toolbar")
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .gap_2()
            .h(px(38.))
            .overflow_x_scroll()
            .child(
                button(format!("Pages ({}) v", self.files.len()), "page-selector")
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_selector(window, cx))),
            )
            .child(
                button("Previous", "previous")
                    .on_click(cx.listener(|this, _, window, cx| this.navigate(-1, window, cx))),
            )
            .child(
                button("Next", "next")
                    .on_click(cx.listener(|this, _, window, cx| this.navigate(1, window, cx))),
            )
            .child(
                button("Reload (F5)", "reload")
                    .on_click(cx.listener(|this, _, window, cx| this.reload(window, cx))),
            )
            .child(
                button("Open in Chrome", "chrome").on_click(cx.listener(|this, _, _, cx| {
                    if let Some(path) = &this.current {
                        cx.open_with_system(path);
                    }
                })),
            );
        for width in viewport::PRESETS {
            controls = controls.child(
                button(width.to_string(), ("preset", width as usize)).on_click(
                    cx.listener(move |this, _, window, cx| this.preset(width, window, cx)),
                ),
            );
        }
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .h(px(viewport::TOOLBAR_HEIGHT))
            .child(open)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_shrink_0()
                            .child(format!("{} x {}", self.viewport.0, self.viewport.1)),
                    )
                    .child(controls),
            )
    }
}
