//! Toolbar dropdown shares the folder page inventory with the sidebar.

use crate::{app::Browser, viewport};
use gpui::{Context, KeyDownEvent, Window, div, prelude::*, px, rgb};

impl Browser {
    pub fn toggle_selector(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selector_open = !self.selector_open;
        if self.selector_open {
            self.selector_focus.focus(window, cx);
            self.selector_scroll.scroll_to_item(self.selected);
        } else {
            self.list_focus.focus(window, cx);
        }
        cx.notify();
    }

    pub fn selector(&self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let mut popup = div()
            .id("page-selector-list")
            .absolute()
            .top(px(viewport::TOOLBAR_HEIGHT))
            .left_0()
            .w_full()
            .max_w(px(600.))
            .max_h(px(320.))
            .flex()
            .flex_col()
            .bg(rgb(0x202834))
            .border_1()
            .border_color(rgb(0x708090))
            .overflow_y_scroll()
            .occlude()
            .track_focus(&self.selector_focus)
            .track_scroll(&self.selector_scroll)
            .on_key_down(cx.listener(Self::selector_key));
        if self.files.is_empty() {
            popup = popup.child(div().p_2().child("Open a folder to select a page."));
        }
        for (index, path) in self.files.iter().enumerate() {
            let label = self.page_label(path);
            popup = popup.child(
                div()
                    .id(("selector-page", index))
                    .flex_shrink_0()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .px_2()
                    .py_1()
                    .bg(rgb(if self.current.as_ref() == Some(path) {
                        0x245b88
                    } else if index == self.selected {
                        0x364152
                    } else {
                        0x202834
                    }))
                    .hover(|style| style.bg(rgb(0x4b5b70)))
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.selected = index;
                        this.activate_selected(window, cx);
                        this.list_focus.focus(window, cx);
                    })),
            );
        }
        popup
    }

    fn selector_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "up" => self.selected = self.selected.saturating_sub(1),
            "down" => self.selected = (self.selected + 1).min(self.files.len().saturating_sub(1)),
            "enter" => {
                self.activate_selected(window, cx);
                self.list_focus.focus(window, cx);
            }
            "escape" => self.toggle_selector(window, cx),
            _ => return,
        }
        self.selector_scroll.scroll_to_item(self.selected);
        cx.stop_propagation();
        cx.notify();
    }
}
