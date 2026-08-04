//! Вклады расширений в статус-бар: разметка и rich-текст.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::probe::registry::probe_area;
use crate::ui::icon::codicon;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;

/// Contributed statusbar item от exthost (StatusBarItemState 1:1).
#[derive(Clone)]
pub struct ContribItem {
    pub id: String,
    /// 1 = Left, 2 = Right.
    pub alignment: u8,
    pub priority: f64,
    pub text: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
    pub color: Option<String>,
    pub visible: bool,
}
impl ContribItem {
    pub fn from_value(v: &serde_json::Value) -> Option<ContribItem> {
        use serde_json::Value;
        Some(ContribItem {
            id: v.get("id").and_then(Value::as_str)?.to_string(),
            alignment: v.get("alignment").and_then(Value::as_u64).unwrap_or(1) as u8,
            priority: v.get("priority").and_then(Value::as_f64).unwrap_or(0.0),
            text: v
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            tooltip: v.get("tooltip").and_then(Value::as_str).map(String::from),
            command: v.get("command").and_then(Value::as_str).map(String::from),
            color: v.get("color").and_then(Value::as_str).map(String::from),
            visible: v.get("visible").and_then(Value::as_bool).unwrap_or(true),
        })
    }
}
/// Текст с `$(icon)`-токенами → children (глифы codicon + текст-куски).
fn rich_text(
    text: &str,
    base: gpui::Rgba,
    group: &SharedString,
    hover: gpui::Rgba,
) -> Vec<AnyElement> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("$(") {
        let (before, after) = rest.split_at(start);
        if !before.is_empty() {
            out.push(div().child(before.to_string()).into_any_element());
        }
        if let Some(end) = after.find(')') {
            let name = &after[2..end];
            if let Some(glyph) = crate::ui::codicon_map::codicon_by_name(name) {
                // `.item:hover { color: text-primary }` наследуется и
                // глифом-codicon (ревью ц.23)
                out.push(
                    codicon(glyph, 12.0)
                        .text_color(base)
                        .group_hover(group.clone(), move |st| st.text_color(hover))
                        .into_any_element(),
                );
            }
            rest = &after[end + 1..];
        } else {
            out.push(div().child(after.to_string()).into_any_element());
            rest = "";
        }
    }
    if !rest.is_empty() {
        out.push(div().child(rest.to_string()).into_any_element());
    }
    out
}
/// Пилюля contributed-элемента: rich-текст + тултип + клик-команда.
pub(crate) fn contrib(it: &ContribItem, p: &Palette) -> AnyElement {
    // `StatusBarItem.color`: литеральная строка проходит как есть, а
    // ThemeColor превращается хостом в `var(--vscode-<id>)`
    // (`exthost/api/status-bar.ts:25-30`). В теме определены ТОЛЬКО три
    // `--vscode-statusBarItem-*`, и единственный из них про передний план —
    // `prominentForeground`; всё остальное — несуществующая переменная, то
    // есть невалидный `color`, и элемент остаётся `--text-muted`.
    // Раньше мы красили любой id палитрой (`deco_color` с фолбэком
    // accent-blue) — там, где оригинал показывает muted, был синий (ревью ц.13).
    let fg = it
        .color
        .as_deref()
        .map(|c| {
            if c == "statusBarItem.prominentForeground" {
                rgba(p.bg_primary)
            } else {
                // `style={{ color }}` пропускает любую CSS-строку: hex,
                // `rgb()`, именованные (ревью ц.17)
                crate::colors::parse_css_color(c, rgba(p.text_muted))
            }
        })
        .unwrap_or(rgba(p.text_muted));
    // `.item:hover { background: bg-surface 60%; color: text-primary }` —
    // ровно как у встроенных элементов ниже. Две копии логики разошлись:
    // здесь стояло `text-primary 8%` и цвет текста не поднимался (ревью ц.7).
    let hover_bg = tint(rgba(p.bg_surface), 0.6);
    let hover_fg = rgba(p.text_primary);
    // .item: padding 0 8, radius-xs (ревью ц.1: был r-sm + py1)
    let group = SharedString::from(format!("sbi-g-{}", it.id));
    let mut el = div()
        .id(SharedString::from(format!("sbi-{}", it.id)))
        .group(group.clone())
        // Свой регион досье 118: раньше и оно, и 116/119/120 указывали на
        // общий `status-bar`, то есть на кроп всей полосы (ревью ц.26)
        .relative()
        .child(probe_area("status-item-contributed"))
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(m::SPACE_2))
        .rounded(px(m::RADIUS_XS))
        .text_color(fg)
        .children(rich_text(&it.text, fg, &group, hover_fg));
    // Тултип — только у КЛИКАБЕЛЬНОГО элемента: некликабельный в оригинале
    // это `<button disabled>`, он гасит pointer-события, и `data-tooltip`
    // не показывается (ревью ц.13)
    if let (Some(tip), true) = (&it.tooltip, it.command.is_some()) {
        el = el.tooltip(crate::ui::tooltip::tooltip(tip.clone()));
    }
    if let Some(cmd) = it.command.clone() {
        el = el
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
                let cmd = cmd.clone();
                std::thread::spawn(move || {
                    if let Some(c) = crate::host_link::client() {
                        let _ = c.request("kamin:command:execute", vec![serde_json::json!(cmd)]);
                    }
                });
            });
    }
    // Кликабельный элемент — таб-стоп (`:focus-visible`)
    if it.command.is_none() {
        return el.into_any_element();
    }
    crate::ui::focus_ring::focusable(
        el,
        &format!("sb:{}", it.id),
        m::RADIUS_XS,
        rgba(p.accent_primary),
    )
    .into_any_element()
}
