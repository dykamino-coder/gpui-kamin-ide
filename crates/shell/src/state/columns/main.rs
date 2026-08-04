//! Колонка чата и её нижняя карта.
//!
//! Блок `render` вынесен как есть (`plan/100-refactor-250.md`).

use crate::host_link::{self};
use crate::probe::registry::probe_area;
use crate::root::DragKind;
use crate::state::model::RootView;
use crate::ui::splitter::h_handle;
use crate::ui::webview_body::gap_wrap;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Window, div, px};
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn main_column(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        p: &'static Palette,
        no_sessions: bool,
        main_split: f32,
        chat_content: AnyElement,
    ) -> AnyElement {
        if self.layout.main_bottom_visible && !no_sessions {
            div()
                .flex()
                .flex_col()
                .size_full()
                .min_w(px(0.))
                .child(
                    div()
                        .h(gpui::relative(main_split)) // без min-h (ревью ц.1)
                        // `.main { flex-shrink: 0 }` — секция не сжимается
                        // соседями (ревью ц.14)
                        .flex_shrink_0()
                        .w_full()
                        .child(gap_wrap(chat_content)),
                )
                .child(h_handle(
                    "main-bottom-handle",
                    p,
                    self.handle_show("main-bottom-handle", DragKind::MainBottom),
                    self.drag.is_some(),
                    0.0,
                    0.0,
                    cx.listener(move |this, e: &gpui::MouseDownEvent, window, cx| {
                        let vw = f32::from(window.viewport_size().width);
                        this.begin_drag(DragKind::MainBottom, f32::from(e.position.y), vw);
                        cx.notify();
                    }),
                    cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hovered_handle = if *hovered {
                            Some("main-bottom-handle")
                        } else {
                            None
                        };
                        cx.notify();
                    }),
                ))
                .child(div().flex_1().min_h(px(0.)).min_w(px(0.)).child(gap_wrap({
                    let slot = crate::activity::PanelSlot::MainBottom;
                    let body = self.tool_body(slot, p, cx);
                    crate::ui::glint::glint_surface_wv_holed(
                        p,
                        div()
                            .id("main-bottom")
                            .relative()
                            .size_full()
                            .child(probe_area("main-bottom"))
                            .children(
                                self.card_drop_hint_top(crate::activity::PanelSlot::MainBottom, p),
                            )
                            .child(crate::ui::slot_panel::slot_panel(
                                slot,
                                self.activity.state(slot),
                                "Left Bottom",
                                crate::ui::panel_placeholder::SlotIcon::MainBottom,
                                true, // нижняя панель → пикер вверх
                                self.tool_drag_over_index(slot),
                                self.tool_dragging_in(slot),
                                body,
                                &self.tx,
                                p,
                            ))
                            .into_any_element(),
                    )
                    .into_any_element()
                })))
                .into_any_element()
        } else {
            gap_wrap(chat_content)
        }
    }

    pub(crate) fn chat_content(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        p: &'static Palette,
        has_active: bool,
        welcome_el: AnyElement,
    ) -> AnyElement {
        if has_active {
            // Main-слот СО СТРИПОМ («Claude Bridge»-таб + «…»), как оригинал
            let slot = crate::activity::PanelSlot::Main;
            let body = self.tool_body(slot, p, cx);
            crate::ui::glint::glint_surface_wv_holed(
                p,
                div()
                    .id("main-slot")
                    .relative()
                    .size_full()
                    .child(probe_area("main"))
                    .children(self.card_drop_hint_top(slot, p))
                    .child(crate::ui::slot_panel::slot_panel(
                        slot,
                        self.activity.state(slot),
                        "Left",
                        crate::ui::panel_placeholder::SlotIcon::Main,
                        false,
                        self.tool_drag_over_index(slot),
                        self.tool_dragging_in(slot),
                        body,
                        &self.tx,
                        p,
                    ))
                    .into_any_element(),
            )
            .into_any_element()
        } else {
            // Welcome занимает всю карту, без стрипа-хедера (как оригинал)
            crate::ui::glint::glint_surface_wv_holed(
                p,
                div()
                    .id("main-slot")
                    .relative()
                    .size_full()
                    .child(welcome_el)
                    .into_any_element(),
            )
            .into_any_element()
        }
    }

    pub(crate) fn welcome(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
        p: &'static Palette,
    ) -> AnyElement {
        {
            crate::ui::welcome::welcome(
                env!("CARGO_PKG_VERSION"),
                self.welcome_glow.clone(),
                move |_w, cx| {
                    let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
                        files: false,
                        directories: true,
                        multiple: false,
                        prompt: None,
                    });
                    cx.spawn(async move |_| {
                        if let Ok(Ok(Some(paths))) = rx.await
                            && let Some(path) = paths.first()
                        {
                            let path = path.to_string_lossy().to_string();
                            std::thread::spawn(move || {
                                if let Some(c) = host_link::client() {
                                    let _ = c.request(
                                        "kamin:sessions:newSessionInFolder",
                                        vec![serde_json::json!(path)],
                                    );
                                }
                            });
                        }
                    })
                    .detach();
                },
                p,
            )
        }
    }
}
