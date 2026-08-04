//! Точка статуса сессии слева от имени.
//!
//! Вынесено из `row.rs` без изменения поведения (`plan/100-refactor-250.md`).

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
    // Под RDP вечный пульс = непрерывный поток кадров в сеть; статичная
    // синяя точка доносит статус не хуже.
    if !working || crate::win_integration::reduce_motion() {
        return dot.into_any_element();
    }
    // working: keyframes `bridgeWorkingPulse` 1.1s — opacity 0.5→1→0.5,
    // scale 1→1.5→1. transform в gpui нет, поэтому «масштаб» — абсолютный
    // внутренний кружок в боксе фиксированных 6px: лейаут не дёргается.
    use gpui::AnimationExt as _;
    div()
        .flex_shrink_0()
        .relative()
        .w(px(dot_size))
        .h(px(dot_size))
        .child(dot.absolute().with_animation(
            SharedString::from(format!("sdot-pulse-{}", s.id)),
            gpui::Animation::new(std::time::Duration::from_millis(1100)).repeat(),
            move |d, delta| {
                let f = (std::f32::consts::PI * delta).sin();
                let sz = dot_size * (1.0 + 0.5 * f);
                let off = (dot_size - sz) / 2.0;
                d.left(px(off))
                    .top(px(off))
                    .w(px(sz))
                    .h(px(sz))
                    .opacity(0.5 + 0.5 * f)
            },
        ))
        .into_any_element()
}
