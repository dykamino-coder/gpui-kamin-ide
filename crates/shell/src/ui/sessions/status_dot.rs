//! Точка статуса сессии слева от имени.
//!
//! Вынесено из `row.rs` (`plan/100-refactor-250.md`). Работающая сессия —
//! статичная точка и время работы, обновляемое раз в секунду (INC-2026-0001).

use crate::colors::rgba;
use gpui::prelude::*;
use gpui::{AnyElement, Rgba, SharedString, div, px};
use kamin_model::Session;
use kamin_theme::Palette;

/// Bridge-статус из metadata (VSIX пишет `bridgeStatus`/`bridgeWorking`):
/// working=blue 6px, connected=green, connecting=yellow, error=red,
/// disconnected=muted; нет статуса → активный tab-color / muted.
pub(crate) fn status_dot(s: &Session, is_active: bool, tab_color: Rgba, p: &Palette) -> AnyElement {
    let meta = s.metadata.as_ref();
    let working = meta
        .and_then(|m| m.get("bridgeWorking"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let bridge = meta
        .and_then(|m| m.get("bridgeStatus"))
        .and_then(|v| v.as_str());
    let (dot_color, dot_size, status_tip) = if working {
        (rgba(p.accent_blue), 6.0, Some("Working…"))
    } else {
        match bridge {
            Some("connected") => (rgba(p.accent_green), 4.0, Some("Online")),
            Some("connecting") => (rgba(p.accent_yellow), 4.0, Some("Connecting…")),
            Some("error") => (rgba(p.accent_red), 4.0, Some("Error")),
            Some("disconnected") => (rgba(p.text_muted), 4.0, Some("Offline")),
            _ => (
                if is_active {
                    tab_color
                } else {
                    rgba(p.text_muted)
                },
                4.0,
                None,
            ),
        }
    };
    let dot = div()
        .id(SharedString::from(format!("sdot-{}", s.id)))
        .flex_shrink_0()
        .w(px(dot_size))
        .h(px(dot_size))
        .rounded_full()
        .bg(dot_color)
        .when_some(status_tip, |d, tip| {
            d.tooltip(crate::ui::tooltip::tooltip(tip))
        });
    if !working {
        forget_working(&s.id);
        return dot.into_any_element();
    }
    // working: статичная точка и время работы рядом. Подпись меняется раз в
    // секунду по общему такту (`anim_tick`): все таймеры — в одном кадре, а
    // прежний пульс `bridgeWorkingPulse` перерисовывал окно на каждой
    // развёртке, пока Claude работает (INC-2026-0001).
    let _ = crate::ui::anim_tick::step(1000);
    let since = working_since(&s.id);
    let label =
        crate::ui::anim_tick::elapsed_label(crate::ui::anim_tick::now().saturating_sub(since));
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(dot)
        .child(
            div()
                .text_size(px(kamin_metrics::FS_XS))
                .text_color(rgba(p.text_muted))
                .child(label),
        )
        .into_any_element()
}

/// Когда сессия начала работать (мс общих часов `anim_tick`). В метаданных
/// только флаг `bridgeWorking`, поэтому момент — первый рендер с флагом.
static WORKING_SINCE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, u64>>,
> = std::sync::LazyLock::new(Default::default);

fn working_since(id: &str) -> u64 {
    let mut map = WORKING_SINCE.lock().unwrap_or_else(|e| e.into_inner());
    *map.entry(id.to_string())
        .or_insert_with(crate::ui::anim_tick::now)
}

fn forget_working(id: &str) {
    if let Ok(mut map) = WORKING_SINCE.lock() {
        map.remove(id);
    }
}
