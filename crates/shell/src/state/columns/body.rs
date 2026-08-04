//! Тело кадра: строка из колонок и ручек сплиттеров.
//!
//! Вынесено из `render` как есть (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use crate::probe::registry::probe_area;
use crate::root::DragKind;
use crate::state::model::RootView;
use crate::ui::activity_bar::{ActivityEntry, activity_bar};
use crate::ui::sessions_list::sessions_sidebar;
use crate::ui::splitter::v_handle;
use crate::ui::webview_body::gap_wrap;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Window, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

impl RootView {
    /// Тело окна: [activity][sidebar][chat][file][right] с ручками
    /// сплиттеров. Аргументы — уже собранные колонки и их обёртки.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn body_row(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        p: &'static Palette,
        main_wrap: AnyElement,
        main_file_handle: AnyElement,
        file_wrap: AnyElement,
        file_right_handle: AnyElement,
        right_wrap: AnyElement,
        _main_column_present: bool,
        _file_fills: bool,
        right_fills: bool,
        sidebar_w: f32,
        log_filter_focused: bool,
        design_input_focused: bool,
        no_sessions: bool,
    ) -> AnyElement {
        div()
            .id("body")
            .relative()
            .flex_1()
            .min_h(px(0.))
            .flex()
            // .body padding 0 4: гуттеры С ОБЕИХ сторон (ревью ц.1)
            .pl(px(m::BODY_GUTTER_X))
            .pr(px(m::BODY_GUTTER_X))
            // `.body { overflow: hidden }` — горизонтальное
            // переполнение колонок режет каркас (ревью ц.13)
            .overflow_hidden()
            .child(probe_area("body"))
            // `ActivityBar.tsx:50`: `if (slot === "sidebar" &&
            // !sidebarVisible.value) return null` — при скрытом
            // сайдбаре бар слота не рисуется целиком (его gear
            // переезжает в титлбар, см. TitlebarQuickActions).
            .when(self.sidebar_visible, |body| {
                body.child({
                    let tx = self.tx.clone();
                    let tx_gear = self.tx.clone();
                    activity_bar(
                        &{
                            // Плитки = pinned Sidebar-слота (модель)
                            let st = self.activity.state(crate::activity::PanelSlot::Sidebar);
                            st.pinned
                                .iter()
                                .filter_map(|id| {
                                    // `lookup` — только builtin: запиненный
                                    // contributed-тул выпадал из бара
                                    // (ревью ц.13). Оригинал берёт из
                                    // общего реестра (`ActivityBar.tsx:77`).
                                    crate::activity::lookup_any(id).map(|(_, icon)| ActivityEntry {
                                        id: crate::activity::static_id(id),
                                        icon: crate::activity::intern(&icon),
                                    })
                                })
                                .collect::<Vec<_>>()
                        },
                        &self.tx,
                        // В Customize активен ТОЛЬКО gear — плитки тулов
                        // не подсвечиваются (оригинал)
                        if self.cz.customize_open {
                            None
                        } else {
                            Some(self.sidebar_activity)
                        },
                        self.cz.customize_open,
                        self.tool_drag_over_index(crate::activity::PanelSlot::Sidebar),
                        self.drop_blocked(crate::activity::PanelSlot::Sidebar),
                        self.tool_dragging_in(crate::activity::PanelSlot::Sidebar),
                        p,
                        move |id| {
                            let _ = tx.try_send(ShellEvent::ActivityClicked(id));
                        },
                        move || {
                            let _ = tx_gear.try_send(ShellEvent::Cz(CzEvent::ToggleCustomize));
                        },
                        {
                            let tx = self.tx.clone();
                            move |x, y| {
                                let _ = tx.try_send(ShellEvent::OpenToolPicker(
                                    crate::activity::PanelSlot::Sidebar,
                                    x,
                                    y,
                                    false,
                                ));
                            }
                        },
                    )
                })
            })
            // `AppLayout.tsx:31`: `sidebarShown = sidebarVisible ||
            // inCustomize` — в Customize сайдбар с навигацией остаётся,
            // даже когда в рабочей области он скрыт; гаснет только
            // полоса тулов выше (её условие — чистый `sidebar_visible`).
            .when(self.sidebar_visible || self.cz.customize_open, |body| {
                body.child(
                    div()
                        .relative()
                        // `gap_wrap` даёт px(4) с двух сторон, а у
                        // оригинала зазор берётся с `.body { gap }` —
                        // иначе тело сайдбара уже на 8px (замер ц.8)
                        .w(px(sidebar_w + 8.0))
                        // `.sidebar { flex-shrink: 1; min-width: 100 }`
                        // (`Sidebar.module.css:10`, `Sidebar.tsx:55`):
                        // при нехватке ширины сайдбар ужимается, а не
                        // выталкивает соседей (ревью ц.13)
                        .flex_shrink()
                        .min_w(px(m::PANEL_MIN_SIZE))
                        .h_full()
                        .child(probe_area("sidebar"))
                        .children(self.card_drop_hint_top(crate::activity::PanelSlot::Sidebar, p))
                        .child(gap_wrap(if self.cz.customize_open {
                            // Customize: ПЛОСКИЙ сайдбар без карточки/
                            // бордера (как оригинал; glint-карта убрана)
                            crate::ui::customize::customize_nav(
                                self.cz.customize_panel,
                                &self.cz.customize_pages,
                                self.cz.customize_contrib.as_deref(),
                                &self.cz.customize_contrib_collapsed,
                                &self.tx,
                                p,
                            )
                        } else if self.sidebar_activity == "projects" {
                            sessions_sidebar(
                                self.sessions.as_ref(),
                                &self.collapsed_projects,
                                &self.inactive_open,
                                self.renaming_session
                                    .as_deref()
                                    .zip(self.rename_input.as_ref()),
                                self.hover_pill.as_deref(),
                                &self.tx,
                                p,
                            )
                        } else {
                            // Оригинал `Sidebar.tsx:81-85`: тело
                            // сайдбара — `ActivityBody` активного тула
                            // (дерево, расширения, проблемы, терминал,
                            // contributed), иначе «No tool selected»
                            self.tool_body(crate::activity::PanelSlot::Sidebar, p, cx)
                                .unwrap_or_else(|| {
                                    crate::ui::panel_placeholder::activity_placeholder(
                                        "circle-large",
                                        "No tool selected",
                                        p,
                                    )
                                })
                        })),
                )
                .child(v_handle(
                    "sidebar-handle",
                    p,
                    self.handle_show("sidebar-handle", DragKind::Sidebar),
                    self.drag.is_some(),
                    cx.listener(move |this, e: &gpui::MouseDownEvent, window, cx| {
                        let vw = f32::from(window.viewport_size().width);
                        this.begin_drag(DragKind::Sidebar, f32::from(e.position.x), vw);
                        cx.notify();
                    }),
                    cx.listener(move |this, hovered: &bool, _, cx| {
                        this.hovered_handle = if *hovered {
                            Some("sidebar-handle")
                        } else {
                            None
                        };
                        cx.notify();
                    }),
                ))
            })
            .map(|body| self.with_customize_area(body, p, log_filter_focused, design_input_focused))
            .when(!self.cz.customize_open, {
                // Панели чтят layout-тумблеры (Layout panels поповер);
                // welcome живёт внутри main-карты (chat без сессии)
                let (mv, fv, rv) = (
                    self.layout.main_visible,
                    self.layout.file_panel_visible && !no_sessions,
                    self.layout.right_panel_visible && !no_sessions,
                );
                move |body| {
                    body.when(mv, |b| b.child(main_wrap))
                        .when(fv, |b| b.child(main_file_handle).child(file_wrap))
                        // `{!fill && <div className={resizeHandle}…>}`
                        // (`RightPanel.tsx:113`): в fill-режиме тянуть
                        // нечего, ручка не рендерится (ревью ц.26)
                        .when(rv, |b| {
                            b.when(!right_fills, |b| b.child(file_right_handle))
                                .child(right_wrap)
                        })
                }
            })
            .into_any_element()
    }
}
