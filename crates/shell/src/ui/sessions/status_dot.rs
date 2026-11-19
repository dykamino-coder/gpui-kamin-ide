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
    // секунду по общему такту (`anim_tick`), а прежний пульс
    // `bridgeWorkingPulse` перерисовывал окно на каждой развёртке, пока Claude
    // работает (INC-2026-0001). Секундный шаг перерисовывает только подпись
    // (`ui::ticking`), а не всё окно (INC-2026-0064).
    let since = working_since(&s.id);
    let elapsed = crate::ui::anim_tick::now().saturating_sub(since);
    // Коробка подписи — по шаблону текущей длины; к смене длины («9s» →
    // «10s») родитель раскладывается заново.
    if let Some(at) = next_shape_change(elapsed) {
        crate::ui::anim_tick::repaint_at(since + at);
    }
    let color = rgba(p.text_muted);
    let text = move |label: String| {
        // Без переноса: при пропорциональных цифрах подпись бывает на
        // пиксель шире шаблона и иначе ломалась бы на две строки в коробке.
        div()
            .whitespace_nowrap()
            .text_size(px(kamin_metrics::FS_XS))
            .text_color(color)
            .child(label)
    };
    let sizer = text(shape(&crate::ui::anim_tick::elapsed_label(elapsed))).invisible();
    let label = crate::ui::ticking::ticking(
        SharedString::from(format!("sdot-time-{}", s.id)),
        1000,
        move |_| {
            let elapsed = crate::ui::anim_tick::now().saturating_sub(since);
            text(crate::ui::anim_tick::elapsed_label(elapsed)).into_any_element()
        },
    )
    .flex_shrink_0()
    .sizer(sizer);
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(dot)
        .child(label)
        .into_any_element()
}

/// Шаблон подписи: цифры — нулями (ширина та же при моноширинных цифрах).
fn shape(label: &str) -> String {
    label
        .chars()
        .map(|c| if c.is_ascii_digit() { '0' } else { c })
        .collect()
}

/// Когда (мс от старта) у подписи `elapsed_label` меняется число знаков.
fn next_shape_change(elapsed: u64) -> Option<u64> {
    const AT: [u64; 5] = [10_000, 60_000, 600_000, 3_600_000, 36_000_000];
    AT.into_iter().find(|&at| at > elapsed)
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

/// Забыть момент старта у сессий, которых больше нет в списке (удалённых).
pub(crate) fn retain_known<'a>(ids: impl IntoIterator<Item = &'a str>) {
    let known: std::collections::HashSet<&str> = ids.into_iter().collect();
    if let Ok(mut map) = WORKING_SINCE.lock() {
        map.retain(|id, _| known.contains(id.as_str()));
    }
}

fn forget_working(id: &str) {
    if let Ok(mut map) = WORKING_SINCE.lock() {
        map.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::{next_shape_change, shape};

    #[test]
    fn the_label_box_changes_only_with_the_label_length() {
        assert_eq!(shape("1:05"), "0:00");
        assert_eq!(next_shape_change(9_999), Some(10_000));
        assert_eq!(next_shape_change(10_000), Some(60_000));
        assert_eq!(next_shape_change(36_000_000), None);
    }
}
