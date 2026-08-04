//! Customize-режим (CustomizeMode + CustomizePanel 1:1, builtin-часть):
//! сайдбар = навигация (Settings/Design/Extensions/Logs/System), main-область
//! = панель с заголовком и телом. Settings — реальные prefs хоста
//! (kamin:prefs:get/set): background-тосты + ConPTY DLL. Design/Logs/System/
//! Extensions — заглушки до своих фаз.

pub use crate::ui::cz::nav::{PANELS, customize_nav};
pub use crate::ui::cz::page::contrib_page;

use crate::ui::cz::page::page_header;
use crate::ui::cz::page::section;
use crate::ui::cz::page::title_for;
use crate::ui::cz::prefs::legacy_bridge_card;
use crate::ui::cz::prefs::pref_row;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::rgba;
use crate::host_link::ShellEvent;

/// Панель Customize в main-области. log_filter — инпут+значение фильтра
/// Logs/System (создаётся в root, как палитра).
#[allow(clippy::too_many_arguments)]
pub fn customize_panel(
    active: &str,
    prefs: Option<(bool, bool)>,
    extensions: Option<&Vec<crate::ui::extensions_panel::ExtDesc>>,
    // Кэш иконок расширений (id → data-URL), как `iconCache` оригинала
    ext_icons: &std::collections::HashMap<String, Option<String>>,
    output: &crate::output_log::OutputChannels,
    system_log: &[crate::output_log::SysEntry],
    log_filter: Option<(&gpui::Entity<gpui_component::input::InputState>, String)>,
    // Каретка в поле поиска логов — :focus-within рамки (ревью ц.23)
    log_filter_focused: bool,
    // Скролл тела Logs: stick-to-bottom (ревью ц.23)
    logs_scroll: &gpui::ScrollHandle,
    syslog_level: &'static str,
    ext_status: &str,
    legacy_fp: Option<&crate::legacy_bridge::BridgeFootprint>,
    legacy_busy: bool,
    design: &crate::ui::design_samples::DesignState,
    // Живой семпл-инпут Design-страницы + его фокус (`.input:focus`)
    design_input: Option<&gpui::Entity<gpui_component::input::InputState>>,
    design_input_focused: bool,
    // Светлая тема: у части семплов отдельные `[data-theme=light]`-состояния
    light: bool,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let (title, subtitle) = title_for(active);
    let body: AnyElement = match active {
        "settings" => {
            let loaded = prefs.is_some();
            let (toasts, conpty) = prefs.unwrap_or((true, false));
            div()
                .when_some(legacy_fp.and_then(|fp| legacy_bridge_card(fp, legacy_busy, tx, p)), |d, card| d.child(card))
                .flex()
                .flex_col()
                // `.root { gap: var(--space-4) }`
                .gap(px(m::SPACE_4))
                .child(section("Notifications", vec![pref_row(
                    "pref-toasts",
                    "backgroundToasts",
                    "Show background notifications when KaminIDE is not focused",
                    "Raises a native, always-on-top toast when Claude is waiting for an answer or a session just finished and the window is minimised or in the background.",
                    toasts,
                    loaded,
                    tx,
                    p,
                )], p))
                .child(section("Terminal", vec![pref_row(
                    "pref-conpty",
                    "useConptyDll",
                    "Use the system ConPTY DLL (Windows-signed)",
                    "Off (default) uses node-pty's bundled ConPTY — instant on desktop Windows 11. Turn on if corp AppLocker blocks the bundled (unsigned) OpenConsole.exe and shell spawns hang. Re-open the terminal to apply.",
                    conpty,
                    loaded,
                    tx,
                    p,
                )], p))
                .into_any_element()
        }
        "design" => crate::ui::design_panel::design_panel(
            design,
            design_input,
            design_input_focused,
            light,
            tx,
            p,
        ),
        "extensions" => {
            crate::ui::extensions_panel::extensions_panel(extensions, ext_icons, ext_status, tx, p)
        }
        "logs" => crate::ui::logs_panel::logs_panel(
            output,
            log_filter,
            log_filter_focused,
            logs_scroll,
            tx,
            p,
        ),
        "system" => crate::ui::logs_panel::system_panel(
            system_log,
            log_filter,
            log_filter_focused,
            syslog_level,
            tx,
            p,
        ),
        // `ComingSoon` (`CustomizePanel.tsx:80-87`): fa-screwdriver-wrench 32
        // op .5 + «Phase B»; `.placeholder` gap space-2 / padding space-7
        _ => div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(m::SPACE_2))
            .p(px(m::SPACE_7))
            .text_color(rgba(p.text_muted))
            .child(
                crate::ui::icon::fa("\u{f7d9}", 32.0)
                    .w(px(32.0))
                    .h(px(32.0))
                    .opacity(0.5),
            )
            .child("Phase B")
            .into_any_element(),
    };

    // Оригинал CustomizePanel.module.css:
    //   .header { padding: 20 24 12; border-bottom: 1px color-mix(bg-overlay 30%) }
    //   .body   { flex: 1; overflow-y: auto; padding: 16 24 }
    div()
        .size_full()
        .flex()
        .flex_col()
        .min_h(px(0.))
        .child(page_header(
            SharedString::from(title),
            SharedString::from(subtitle),
            p,
        ))
        .child(
            div()
                .relative()
                // Поэлементный кроп страниц Customize (parity/shots_drive.py)
                .child(crate::probe::registry::probe_area("cz-body"))
                .id("cz-body-scroll")
                .flex_1()
                .min_h(px(0.))
                // `.body { overflow-y: auto }`
                .overflow_y_scroll()
                .py(px(m::SPACE_4))
                .px(px(m::SPACE_6))
                .child(body),
        )
        .into_any_element()
}
