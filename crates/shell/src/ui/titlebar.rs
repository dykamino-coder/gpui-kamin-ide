//! Собственный титлбар 42px — порядок кластеров ДОСЛОВНО из Titlebar.tsx:
//! [brand: kaminoid 26px][quick-actions: sidebar-toggle (+gear при скрытом
//! сайдбаре)][табы сессий][drag][search-пилюля «Type a command…»]
//! [LayoutToggles fa-table-columns][ThemeQuickToggle fa-moon]
//! [DevTools fa-bug + label][min][max][close].
//! Кнопка контролов: 36×36 КРУГ; quick-action: 28×28 radius 8.

use gpui::prelude::*;
use gpui::{AnyElement, App, Window, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

use crate::colors::rgba;

// Армирование драга титлбара: down запоминает точку, move с порогом 4px
// запускает нативный caption-drag (клик в maximized не restore-ит окно).
thread_local! {
    static DRAG_ARM: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
}
use crate::probe::registry::probe_area;

pub struct TitlebarState {
    pub sidebar_visible: bool,
    /// Ширина сайдбара: `.leftCluster` пиннится к ней инлайновым стилем
    /// (`Titlebar.tsx:35`), при скрытом сайдбаре — `auto`.
    pub sidebar_width: f32,
    /// Глиф темы: moon (dark) / sun (light) / half (contributed).
    pub theme_glyph: &'static str,
    /// Поповер Layout panels открыт (active-подсветка триггера).
    pub layout_popover_open: bool,
    /// Customize-режим активен (active-подсветка gear).
    pub customize_open: bool,
    /// Дропдаун «+» (folder/no-folder) открыт.
    pub on_toggle_sidebar: Box<dyn Fn()>,
    pub on_open_palette: Box<dyn Fn()>,
    pub tx: smol::channel::Sender<crate::host_link::ShellEvent>,
}

/// Титлбар (см. модульный док-коммент).
pub fn titlebar(
    p: &Palette,
    state: TitlebarState,
    // `SessionTabs` при НУЛЕ сессий возвращает `null` целиком
    // (`SessionTabs.tsx:98`): ни чипов, ни «+», ни спейсера. У нас «+»
    // висел всегда — здесь это `None` (ревью ц.35)
    tabs: Option<AnyElement>,
    window: &Window,
    _cx: &App,
) -> impl IntoElement {
    // Колбэки — `Box<dyn Fn()>`, они не клонируются: забираем их из
    // состояния и отдаём тем частям, которым они нужны.
    let mut state = state;
    let toggle = std::mem::replace(&mut state.on_toggle_sidebar, Box::new(|| {}));
    let open_palette = std::mem::replace(&mut state.on_open_palette, Box::new(|| {}));

    div()
        .id("titlebar")
        .relative()
        // Драг окна: ТОЛЬКО client-side start_window_move на bubble-клике
        // (пустоты/лого/гапы — интерактивные дети делают stop_propagation).
        // ⚠ window_control_area(Drag) здесь НЕЛЬЗЯ: gpui-Windows СЪЕДАЕТ
        // mouse-UP в Drag-зоне (down/move проходят) → клики и drag-reorder
        // чипов никогда не завершаются. Доказано probe-кликами 2026-07-25.
        .on_mouse_down(
            gpui::MouseButton::Left,
            |e: &gpui::MouseDownEvent, window, _| {
                if e.click_count >= 2 {
                    // Двойной клик по титлбару — тот же тоггл, что у кнопки:
                    // `zoom_window()` только разворачивает
                    let _ = window;
                    crate::overlay::toggle_main_maximize();
                } else {
                    // Драг НЕ с down (клик по хедеру в maximized не должен
                    // restore-ить окно) — армируем, драг стартует по move
                    // с порогом (см. on_mouse_move ниже)
                    DRAG_ARM
                        .with(|a| a.set(Some((f32::from(e.position.x), f32::from(e.position.y)))));
                }
            },
        )
        .on_mouse_up(gpui::MouseButton::Left, |_, _, _| {
            DRAG_ARM.with(|a| a.set(None));
        })
        .on_mouse_move(|e: &gpui::MouseMoveEvent, _, _| {
            if e.pressed_button != Some(gpui::MouseButton::Left) {
                return;
            }
            if let Some((sx, sy)) = DRAG_ARM.with(std::cell::Cell::get) {
                let dx = f32::from(e.position.x) - sx;
                let dy = f32::from(e.position.y) - sy;
                if dx * dx + dy * dy >= 16.0 {
                    DRAG_ARM.with(|a| a.set(None));
                    // gpui start_window_move на Windows — заглушка; шлём
                    // системе настоящий caption-drag (работает и maximized)
                    crate::overlay::start_native_window_drag();
                }
            }
        })
        .flex_shrink_0()
        .h(px(m::TITLEBAR_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .text_size(px(m::FS_SM))
        .text_color(rgba(p.text_muted))
        .child(probe_area("titlebar"))
        .child(crate::ui::titlebar_parts::left_cluster(
            p, &state, window, toggle,
        ))
        .child(crate::ui::titlebar_parts::tabs_slot(
            p, &state, window, tabs,
        ))
        .children(crate::ui::titlebar_right::search_and_actions(
            p,
            &state,
            window,
            open_palette,
        ))
        .child(crate::ui::titlebar_right::window_controls(
            p, &state, window,
        ))
}
