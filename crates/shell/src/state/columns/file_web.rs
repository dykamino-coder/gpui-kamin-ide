//! Верх файловой колонки в веб-режиме: панель браузера.
//!
//! Страницу рисует CEF обычным элементом кадра — ни дыры в фоне, ни отдельного
//! окна под неё больше нет.

use crate::state::model::RootView;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Focusable, IntoElement, Window, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn file_top_web(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        p: &'static Palette,
    ) -> AnyElement {
        // Адрес берём ИЗ строки адреса, а не из константы: раньше в поле
        // стоял один сайт, а грузился другой — расхождение сбивало с толку.
        let start_url = self
            .browser_input
            .as_ref()
            .map(|inp| crate::ui::browser_pane::normalize_url(&inp.read(cx).value()))
            .filter(|u| u != "about:blank")
            .unwrap_or_else(|| crate::web::DEMO_URL.to_string());
        crate::web::open("browser", &start_url, 800, 600, window.scale_factor());

        let viewport = div()
            .relative()
            .flex_1()
            .min_h(px(0.))
            .child(crate::probe::registry::probe_area("browser-viewport"))
            .child(crate::web::web_view("browser", m::RADIUS_MD))
            .into_any_element();

        let addr_focused = self
            .browser_input
            .as_ref()
            .is_some_and(|inp| inp.read(cx).focus_handle(cx).is_focused(window));
        self.browser_select_all_on_focus(addr_focused, window, cx);
        match &self.browser_input {
            Some(inp) => crate::ui::browser_pane::visual_frame(inp, addr_focused, p, viewport),
            None => viewport,
        }
    }
}
