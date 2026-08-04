//! Файловая колонка: дерево, редактор, веб-режим, нижняя карта.
//!
//! Блок `render` вынесен как есть (`plan/100-refactor-250.md`).

use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use crate::probe::registry::probe_area;
use crate::root::DragKind;
use crate::state::model::RootView;
use crate::ui::splitter::h_handle;
use crate::ui::webview_body::gap_wrap;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Window, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

impl RootView {
    pub(crate) fn file_column(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        p: &'static Palette,
        viewport_h: f32,
        file_bottom_px: f32,
    ) -> AnyElement {
        {
            let file_mode = self.layout.file_panel_mode.clone();
            // Центр — редакторный слот: web-браузер / открытый файл / пустой
            // плейсхолдер «File». Дерево тут НЕ показываем (оно живёт в
            // right-top; центр — всегда редактор, как в оригинале).
            let top_content: AnyElement = if file_mode == "web" {
                self.file_top_web(window, cx, p)
            } else if !self.ed.editor_tabs.is_empty() {
                self.file_top_editor(window, cx, p, viewport_h)
            } else {
                self.file_top_tree(window, cx, p)
            };
            // Верхняя карта: modeHeader (justify-end, pad 6/8/0) + контент.
            // Внешний drop файла (из Explorer) → открыть в редакторе.
            let top_card = div()
                .flex()
                .flex_col()
                .size_full()
                .min_h(px(0.))
                .on_drop(cx.listener(|this, paths: &gpui::ExternalPaths, _, cx| {
                    for path in paths.paths() {
                        if path.is_file() {
                            let _ = this.tx.try_send(ShellEvent::Ed(EdEvent::OpenFile(
                                path.display().to_string(),
                            )));
                        }
                    }
                    cx.notify();
                }))
                // Внутренний drag из дерева → открыть файл
                .on_drop(
                    cx.listener(|this, f: &crate::ui::file_list::DraggedFile, _, cx| {
                        // Мультивыбор открывается целиком (`dragPaths`, ц.24)
                        for path in &f.paths {
                            let _ = this
                                .tx
                                .try_send(ShellEvent::Ed(EdEvent::OpenFile(path.clone())));
                        }
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .items_center()
                        .flex_shrink_0()
                        .pt(px(6.0))
                        .px(px(m::SPACE_2))
                        .child(crate::ui::file_panel_tabs::file_panel_mode_tabs(
                            &file_mode, &self.tx, p,
                        )),
                )
                .child(top_content);
            // Гейт filePanelBottomVisible: низ скрыт → верхняя карта на всю
            // высоту, сплиттер и centralBottom не рендерятся (как оригинал)
            let file_bottom_on = self.layout.file_panel_bottom_visible;
            let mut file_col = div().flex().flex_col().size_full().min_w(px(0.)).child(
                div()
                    // `.topCard { flex: 1 }` — верх ВСЕГДА остаток
                    // (ревью ц.19: раньше он вёл долей, а низ был остатком)
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .child(gap_wrap(
                        crate::ui::glint::glint_surface_wv_holed(p, top_card.into_any_element())
                            .into_any_element(),
                    )),
            );
            if file_bottom_on {
                file_col = file_col
                    .child(h_handle(
                        "file-bottom-handle",
                        p,
                        self.handle_show("file-bottom-handle", DragKind::FileBottom),
                        self.drag.is_some(),
                        0.0,
                        0.0,
                        cx.listener(move |this, e: &gpui::MouseDownEvent, window, cx| {
                            let vw = f32::from(window.viewport_size().width);
                            this.begin_drag(DragKind::FileBottom, f32::from(e.position.y), vw);
                            cx.notify();
                        }),
                        cx.listener(move |this, hovered: &bool, _, cx| {
                            this.hovered_handle = if *hovered {
                                Some("file-bottom-handle")
                            } else {
                                None
                            };
                            cx.notify();
                        }),
                    ))
                    .child({
                        // Слот centralBottom: стрип-панель (как у центральных
                        // нижних оригинала), консоль = тул слота
                        let slot = crate::activity::PanelSlot::CentralBottom;
                        let body = self.tool_body(slot, p, cx);
                        // `style={{ height: Npx, flexShrink: 0 }}`
                        // (`FilePanel.tsx:145`) — низ ведёт, верх остаток
                        div()
                            .h(px(file_bottom_px))
                            .flex_shrink_0()
                            // Явная ширина, как у верхней карты: без w_full
                            // taffy при определённых размерах давал карте
                            // auto→0 (репро юзера: «нижний блок пропал»,
                            // probe: central-bottom w=0 при живом верхе).
                            .w_full()
                            .min_w(px(0.))
                            .child(gap_wrap(
                                crate::ui::glint::glint_surface_wv_holed(
                                    p,
                                    div()
                                        .id("central-bottom")
                                        .relative()
                                        .size_full()
                                        .child(probe_area("central-bottom"))
                                        .children(self.card_drop_hint_top(
                                            crate::activity::PanelSlot::CentralBottom,
                                            p,
                                        ))
                                        .child(crate::ui::slot_panel::slot_panel(
                                            slot,
                                            self.activity.state(slot),
                                            "Central Bottom",
                                            crate::ui::panel_placeholder::SlotIcon::CenterBottom,
                                            true,
                                            self.tool_drag_over_index(slot),
                                            self.tool_dragging_in(slot),
                                            body,
                                            &self.tx,
                                            p,
                                        ))
                                        .into_any_element(),
                                )
                                .into_any_element(),
                            ))
                    });
            }
            file_col.into_any_element()
        }
    }
}
