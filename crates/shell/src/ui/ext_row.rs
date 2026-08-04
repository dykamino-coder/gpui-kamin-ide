//! Строка расширения и заголовок группы.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::CzEvent;
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use serde_json::Value;
use smol::channel::Sender;

/// Дескриптор расширения (подмножество ExtensionDescriptor хоста).
#[derive(Clone, Debug)]
pub struct ExtDesc {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub enabled: bool,
    pub active: bool,
    pub activation_error: bool,
    pub builtin: bool,
}
impl ExtDesc {
    /// Разбор элемента ответа kamin:extensions:list.
    pub fn from_value(v: &Value) -> Option<ExtDesc> {
        Some(ExtDesc {
            id: v.get("id")?.as_str()?.to_string(),
            display_name: v
                .get("displayName")
                .and_then(Value::as_str)
                .unwrap_or_else(|| v.get("id").and_then(Value::as_str).unwrap_or(""))
                .to_string(),
            version: v
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            enabled: v.get("enabled").and_then(Value::as_bool).unwrap_or(true),
            active: v.get("active").and_then(Value::as_bool).unwrap_or(false),
            activation_error: v.get("activationError").is_some_and(|e| !e.is_null()),
            builtin: v.get("builtin").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    fn status(&self) -> &'static str {
        if !self.enabled {
            "disabled"
        } else if self.activation_error {
            "activation error"
        } else if self.active {
            "active"
        } else {
            "idle"
        }
    }
}
/// `.iconFallback`: 26×26, кодикон extensions 16, text-muted.
fn fallback_icon(p: &Palette) -> AnyElement {
    div()
        .w(px(26.0))
        .h(px(26.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_color(rgba(p.text_muted))
        .child(crate::ui::icon::codicon("\u{eae6}", 16.0))
        .into_any_element()
}
pub(crate) fn ext_row(
    e: &ExtDesc,
    icon: Option<&Option<String>>,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let hover_bg = tint(rgba(p.bg_surface), 0.6);
    // .toggle: Enable/Disable-кнопка (не switch-пилюля)
    let toggle_btn = {
        let tx = tx.clone();
        let ext_id = e.id.clone();
        let on = e.enabled;
        div()
            .id(SharedString::from(format!("extt-{}", e.id)))
            .flex_shrink_0()
            .px(px(10.0))
            .py(px(2.0))
            .rounded(px(m::RADIUS_SM))
            .border_1()
            .border_color(tint(rgba(p.text_muted), 0.3))
            .bg(rgba(p.bg_surface))
            .text_size(px(m::FS_XS))
            .text_color(rgba(p.text_primary))
            .cursor_pointer()
            .hover({
                let hb = rgba(p.bg_overlay);
                move |s| s.bg(hb)
            })
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Cz(CzEvent::ToggleExtension(
                    ext_id.clone(),
                    !on,
                )));
            })
            .child(if e.enabled { "Disable" } else { "Enable" })
    };
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(m::SPACE_2))
        .p(px(m::SPACE_2))
        .rounded(px(m::RADIUS_SM))
        .hover(move |s| s.bg(hover_bg))
        .child(
            // `.icon` 26×26 r-xs object-fit contain — data-URL из
            // `kamin:extensions:icon`; нет иконки → `.iconFallback`
            // (`ExtensionsPanel.module.css:68-76`)
            match icon
                .and_then(|u| u.as_deref())
                .and_then(crate::ui::icon::data_uri_image)
            {
                Some(img) => gpui::img(img)
                    .w(px(26.0))
                    .h(px(26.0))
                    .flex_shrink_0()
                    .rounded(px(m::RADIUS_XS))
                    .into_any_element(),
                None => fallback_icon(p),
            },
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .id(SharedString::from(format!("extn-{}", e.id)))
                        .w_full()
                        .text_size(px(m::FS_SM))
                        .text_color(rgba(p.text_primary))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .tooltip(crate::ui::tooltip::tooltip(e.id.clone()))
                        .child(e.display_name.clone()),
                )
                .child(
                    div()
                        .text_size(px(m::FS_XS))
                        .text_color(rgba(p.text_muted))
                        .child(format!("{} · {}", e.version, e.status())),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .flex_shrink_0()
                .child(toggle_btn)
                .when(!e.builtin, |actions| {
                    let tx = tx.clone();
                    let ext_id = e.id.clone();
                    actions.child(
                        // .uninstall: 24×22, hover red 16% + red
                        div()
                            .id(SharedString::from(format!("extu-{}", e.id)))
                            .w(px(24.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(m::RADIUS_SM))
                            .text_color(rgba(p.text_muted))
                            .cursor_pointer()
                            .hover({
                                let hb = tint(rgba(p.accent_red), 0.16);
                                move |s| s.bg(hb).text_color(rgba(p.accent_red))
                            })
                            .tooltip(crate::ui::tooltip::tooltip("Uninstall"))
                            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                                cx.stop_propagation();
                                let _ = tx.try_send(ShellEvent::Cz(CzEvent::UninstallExtension(
                                    ext_id.clone(),
                                )));
                            })
                            .child(crate::ui::icon::codicon("\u{ea81}", 16.0)), // trash: своего кегля нет
                    )
                }),
        );
    // .disabled: приглушение строки целиком
    if !e.enabled {
        row = row.opacity(0.55);
    }
    row.into_any_element()
}
/// .groupHeader: uppercase fs-xs 600 muted, «Title — N».
pub(crate) fn group_header(title: &str, n: usize, p: &Palette) -> AnyElement {
    div()
        .px(px(m::SPACE_2))
        .pt(px(m::SPACE_2))
        .pb(px(4.0))
        .text_size(px(m::FS_XS))
        .letter_spacing(px(m::FS_XS * 0.04))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(rgba(p.text_muted))
        .child(format!("{} — {}", title.to_uppercase(), n))
        .into_any_element()
}
