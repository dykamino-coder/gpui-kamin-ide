//! Части титлбара: левый кластер, слот табов, поиск, контролы окна.
//!
//! Куски цепочки перенесены дословно (`plan/100-refactor-250.md`).

use crate::colors::rgba;
use crate::host::events::CzEvent;
use crate::probe::registry::probe_area;
use crate::ui::focus_ring::FocusRing;
use crate::ui::icon::{FA_GEAR, FA_PLUS, fa};
use crate::ui::titlebar::TitlebarState;
use crate::ui::titlebar_buttons::action_button;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, Window, div, img, px, svg};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Левый кластер: логотип и переключатель сайдбара.
#[allow(clippy::too_many_arguments)]
pub(crate) fn left_cluster(
    p: &Palette,
    state: &TitlebarState,
    window: &Window,
    toggle: Box<dyn Fn()>,
) -> AnyElement {
    let _ = window;
    // `.leftCluster` (`Titlebar.module.css:46-52`): h 100%, flex,
    // items-center, flex-shrink 0, overflow hidden; ширина инлайном =
    // `sidebarWidth` при видимом сайдбаре и `auto` при скрытом. Обёртки
    // не было вовсе — бренд и quick-actions висели прямыми детьми
    // корня, а отступ стрипа задавала константа (ревью ц.35)
    div()
        .relative()
        .h_full()
        .flex()
        .items_center()
        .flex_shrink_0()
        .overflow_hidden()
        .child(probe_area("left-cluster"))
        // `sidebarVisible || sidebarMode === "customize"`
        // (`Titlebar.tsx:35`): в режиме Customize кластер остаётся
        // пиннутым к ширине сайдбара
        .when(state.sidebar_visible || state.customize_open, |d| {
            d.w(px(state.sidebar_width))
        })
        .child(
            // .brand: 42×42, лого kaminoid 26px
            div()
                .relative()
                // `-webkit-app-region: no-drag` (`Titlebar.module.css:24`):
                // тяга за логотип НЕ двигает окно (ревью ц.25)
                .occlude()
                .w(px(m::TITLEBAR_HEIGHT))
                .h(px(m::TITLEBAR_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .flex_shrink_0()
                .child(probe_area("brand"))
                .child(
                    img(SharedString::from("icons/kaminoid.svg"))
                        .w(px(26.0))
                        .h(px(26.0)),
                ),
        )
        .child(
            // TitlebarQuickActions: gap 1px, padding 0 8
            div()
                .relative()
                .flex()
                .items_center()
                .gap(px(1.0))
                .px(px(m::SPACE_2))
                .flex_shrink_0()
                .child(probe_area("quick-actions"))
                .child(action_button(
                    "toggle-sidebar",
                    m::ICON_BUTTON_ROUND,
                    m::RADIUS_SM,
                    if state.sidebar_visible {
                        "Hide sidebar"
                    } else {
                        "Show sidebar"
                    },
                    p,
                    state.sidebar_visible,
                    false,
                    move |_, _| toggle(),
                    // ⚠ gpui svg БЕЗ text_color рисует ПУСТО (не наследует
                    // currentColor — пойман юзером), поэтому цвет считаем сами
                    // по тем же правилам, что `TitlebarQuickActions.module.css`:
                    // .btn = text-secondary, .active = text-primary.
                    // `.btn:hover { color: text-primary }` поднимает и SVG —
                    // собственный `.hover()` до него не доходит, поэтому
                    // группа + `group_hover` (ревью ц.19)
                    svg()
                        .path(SharedString::from("icons/panel-left.svg"))
                        .w(px(14.0))
                        .h(px(12.0))
                        .text_color(rgba(if state.sidebar_visible {
                            p.text_primary
                        } else {
                            p.text_secondary
                        }))
                        .group_hover("qa-toggle-sidebar", {
                            let tp = rgba(p.text_primary);
                            move |st| st.text_color(tp)
                        })
                        .into_any_element(),
                ))
                .when(!state.sidebar_visible, |row| {
                    row.child(
                        div()
                            .w(px(1.0))
                            .h(px(14.0))
                            .mx(px(m::SPACE_1))
                            .bg(rgba(p.bg_surface)),
                    )
                    .child(action_button(
                        "customize-gear",
                        m::ICON_BUTTON_ROUND,
                        m::RADIUS_SM,
                        if state.customize_open {
                            "Close Customize"
                        } else {
                            "Customize"
                        },
                        p,
                        state.customize_open,
                        false,
                        {
                            let tx = state.tx.clone();
                            move |_, _| {
                                let _ = tx.try_send(crate::host_link::ShellEvent::Cz(
                                    CzEvent::ToggleCustomize,
                                ));
                            }
                        },
                        fa(FA_GEAR, 12.0).into_any_element(),
                    ))
                }),
        )
        .into_any_element()
}

/// Слот табов сессий: чипы, «+», спейсер-драг.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tabs_slot(
    p: &Palette,
    state: &TitlebarState,
    _window: &Window,
    tabs: Option<AnyElement>,
) -> AnyElement {
    // `.tabsSlot`: flex 1, min-w 0, padding 0 12. Внутри — `.strip` с
    // чипами, затем «+» и только потом спейсер-drag: в
    // `SessionTabs.tsx` кнопка стоит ПЕРЕД `.spacer`, поэтому зазор
    // чип→«+» = её собственный margin 6, а не 12 слота плюс 6.
    div()
        .relative()
        // Ширина слота уходит в реестр: по ней стрип считает бюджет
        // чипов и решает, уводить ли лишние в «N ⌄» (root.rs)
        .child(probe_area("tabs-slot"))
        .flex_1()
        .min_w(px(0.))
        .flex()
        .items_center()
        .h_full()
        .px(px(m::SPACE_3))
        .children(tabs.map(|tabs| {
            // `.strip`: flex 1, min-w 0, скролл чипов скрыт. «+» и
            // спейсер — ЕГО дети (`SessionTabs.tsx:120-134`), а не
            // соседи слота: раньше стрип держался `flex_shrink`, и
            // распорку приходилось компенсировать сиблингом
            div()
                .flex()
                .items_center()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .h_full()
                .child(tabs)
                .child(
                    // «+» в кружочке → дропдаун New session in folder…/Empty session
                    div()
                        .id("new-session")
                        .occlude()
                        .relative()
                        .focus_ring("tb:new-session", 13.0, rgba(p.accent_primary))
                        .child(probe_area("new-session"))
                        .w(px(26.0))
                        .h(px(26.0))
                        // `.newTab` margin 0 6px — и, поскольку кнопка внутри
                        // стрипа перед спейсером, это ВЕСЬ зазор от чипа
                        .mx(px(6.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(rgba(p.bg_surface))
                        .text_color(rgba(p.text_muted))
                        .cursor_pointer()
                        .hover(move |s| {
                            // color-mix(accent 36%, surface) — НЕПРОЗРАЧНЫЙ микс
                            // (alpha-вариант просвечивал титлбар, ревью ц.1)
                            let a = rgba(p.accent_primary);
                            let b = rgba(p.bg_surface);
                            let mix = gpui::Rgba {
                                r: a.r * 0.36 + b.r * 0.64,
                                g: a.g * 0.36 + b.g * 0.64,
                                b: a.b * 0.36 + b.b * 0.64,
                                a: 1.0,
                            };
                            s.bg(mix).text_color(rgba(p.accent_primary))
                        })
                        .tooltip(crate::ui::tooltip::tooltip("New session…"))
                        .on_mouse_down(gpui::MouseButton::Left, {
                            let tx = state.tx.clone();
                            move |e: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                let _ = tx.try_send(
                                    crate::host_link::ShellEvent::ToggleNewSessionMenu(
                                        f32::from(e.position.x),
                                        f32::from(e.position.y),
                                    ),
                                );
                            }
                        })
                        .child(fa(FA_PLUS, 12.0)),
                )
                // `.spacer` flex 1 1 auto, min-w 24 — drag от корня
                .child(div().flex_1().min_w(px(24.0)).h_full())
        }))
        .into_any_element()
}
