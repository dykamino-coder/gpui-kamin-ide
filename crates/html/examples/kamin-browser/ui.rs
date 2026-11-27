//! Plain GPUI chrome and keyboard routing around the unmodified DocumentView.

use crate::{app::Browser, viewport};
use gpui::{
    Context, Focusable, IntoElement, KeyDownEvent, MouseButton, Render, Window, div, prelude::*,
    px, rgb,
};

impl Browser {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut list = div()
            .id("page-list")
            .track_focus(&self.list_focus)
            .flex()
            .flex_col()
            .w(px(viewport::SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .track_scroll(&self.list_scroll)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.list_focus.focus(window, cx)),
            )
            .on_key_down(cx.listener(Self::list_key));
        for (index, path) in self.files.iter().enumerate() {
            let label = self.page_label(path);
            let current = self.current.as_ref() == Some(path);
            let row = div()
                .id(("page", index))
                .flex_shrink_0()
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_middle()
                .min_h(px(28.))
                .px_2()
                .py_1()
                .bg(rgb(if current {
                    0x245b88
                } else if index == self.selected {
                    0x364152
                } else {
                    0x202834
                }))
                .hover(|style| style.bg(rgb(0x364152)))
                .cursor_pointer()
                .child(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.selected = index;
                    this.activate_selected(window, cx);
                    this.list_focus.focus(window, cx);
                }));
            list = list.child(row);
        }
        list
    }

    fn list_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.modifiers.alt || event.keystroke.modifiers.control {
            return;
        }
        match event.keystroke.key.as_str() {
            "up" => self.selected = self.selected.saturating_sub(1),
            "down" => self.selected = (self.selected + 1).min(self.files.len().saturating_sub(1)),
            "enter" => self.activate_selected(window, cx),
            _ => return,
        }
        self.list_scroll.scroll_to_item(self.selected);
        cx.stop_propagation();
        cx.notify();
    }

    fn shortcut(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match (event.keystroke.modifiers.alt, event.keystroke.key.as_str()) {
            (_, "f3") => {
                self.hud.toggle();
                cx.notify();
            }
            (_, "f5") => self.reload(window, cx),
            (_, "f11") => {
                self.preset_width = None;
                window.toggle_fullscreen();
            }
            (true, "left") => self.navigate(-1, window, cx),
            (true, "right") => self.navigate(1, window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for Browser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = if let Some(error) = &self.error {
            div()
                .p_4()
                .text_color(rgb(0x9b2020))
                .child(error.clone())
                .into_any_element()
        } else if let Some(page) = &self.page {
            page.clone().into_any_element()
        } else {
            div()
                .p_4()
                .child(if self.folder.is_some() {
                    "No HTML pages in this folder."
                } else {
                    "Open a local HTML file or folder using the toolbar above."
                })
                .into_any_element()
        };
        #[cfg(feature = "why-profiler")]
        if self.timing_plain {
            return page;
        }
        let mut body = div()
            .flex()
            .w_full()
            .h(px(self.viewport.1))
            .flex_shrink_0()
            .overflow_hidden();
        if self.folder.is_some() {
            body = body.child(self.sidebar(cx));
        }
        body = body.child(
            div()
                .w(px(self.viewport.0))
                .h(px(self.viewport.1))
                .flex_shrink_0()
                .overflow_hidden()
                .bg(rgb(0xffffff))
                .text_color(rgb(0x111111))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        if let Some(page) = &this.page {
                            page.read(cx).focus_handle(cx).focus(window, cx);
                        }
                    }),
                )
                .child(page),
        );
        let mut root = div()
            .id("viewer")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x202834))
            .text_color(rgb(0xf0f2f5))
            .text_size(px(13.))
            .font_family("Segoe UI")
            .capture_key_down(cx.listener(Self::shortcut))
            .child(self.toolbar(window, cx))
            .child(body);
        if self.selector_open {
            root = root.child(self.selector(cx));
        }
        if self.hud.visible {
            root = root.child(self.hud.panel());
        }
        root.into_any_element()
    }
}
