//! Секция пресетов раскладки.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use crate::ui::popover::frame::menu_item;
use crate::ui::popover::frame::menu_label;
use gpui::prelude::*;
use gpui::{div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Секция «Layouts» (LayoutPresetsSection 1:1): Save/Export/Import пункты
/// с codicon-иконками + список пресетов (клик=apply, ПКМ=rename; справа
/// 26x26-кнопки: overwrite, export, star-default, delete).
pub(crate) fn presets_section(tx: &Sender<ShellEvent>, p: &Palette) -> gpui::Div {
    let presets = crate::layout_store::load_presets();
    let mut col = div()
        .flex()
        .flex_col()
        .gap(px(1.0))
        .child(menu_label("Layouts", p))
        .child(menu_item(
            "lpp-save",
            "\u{eb4b}", // codicon-save
            "Save current layout…",
            ShellEvent::OpenSaveLayoutPrompt,
            tx,
            p,
        ))
        .child(menu_item(
            "lpp-export",
            "\u{ea78}", // codicon-desktop-download
            "Export current layout…",
            ShellEvent::ExportPresets,
            tx,
            p,
        ))
        .child(menu_item(
            "lpp-import",
            "\u{eac3}", // codicon-cloud-upload
            "Import layout…",
            ShellEvent::ImportPresets,
            tx,
            p,
        ));
    if presets.is_empty() {
        col = col.child(
            div()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_1))
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .child("No saved layouts yet"),
        );
    }
    for (i, pr) in presets.iter().enumerate() {
        let name = pr.name.clone();
        let is_default = pr.default;
        // .presetIconBtn: 26x26, radius-sm, muted; hover 10% + primary
        let icon_btn = |id: String,
                        glyph: &'static str,
                        active: bool,
                        tip: &'static str,
                        ev: ShellEvent,
                        tx: Sender<ShellEvent>| {
            let hb = tint(rgba(p.text_primary), 0.10);
            div()
                .id(gpui::SharedString::from(id))
                .w(px(26.0))
                .h(px(26.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(m::RADIUS_SM))
                .text_color(if active {
                    rgba(p.accent_primary)
                } else {
                    rgba(p.text_muted)
                })
                .cursor_pointer()
                .hover(move |st| st.bg(hb).text_color(rgba(p.text_primary)))
                .tooltip(crate::ui::tooltip::tooltip(tip))
                .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    let _ = tx.try_send(ev.clone());
                })
                .child(
                    // `.presetIconBtn > i {13px}` (0,1,1) проигрывает вендорному
                    // `.codicon[class*=codicon-]` (0,2,0) — фактический кегль 16 (ревью ц.13)
                    codicon(glyph, 16.0),
                )
        };
        // .presetRow: apply-кнопка растёт, экшены в конце, gap 1px
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(1.0))
                .child(
                    div()
                        .id(gpui::SharedString::from(format!("lpp-{i}")))
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .items_center()
                        .gap(px(m::SPACE_2))
                        .px(px(m::SPACE_3))
                        .py(px(m::SPACE_2))
                        .rounded(px(m::RADIUS_SM))
                        .text_size(px(m::FS_SM))
                        .text_color(rgba(p.text_primary))
                        .cursor_pointer()
                        .hover({
                            let hb = tint(rgba(p.text_primary), 0.10);
                            move |st| st.bg(hb)
                        })
                        .tooltip(crate::ui::tooltip::tooltip(
                            "Apply this layout · right-click to rename",
                        ))
                        .on_mouse_down(gpui::MouseButton::Left, {
                            let tx = tx.clone();
                            let name = name.clone();
                            move |_, _, cx| {
                                cx.stop_propagation();
                                let _ = tx.try_send(ShellEvent::ApplyLayoutPreset(name.clone()));
                            }
                        })
                        .on_mouse_down(gpui::MouseButton::Right, {
                            let tx = tx.clone();
                            let name = name.clone();
                            move |_, _, cx| {
                                cx.stop_propagation();
                                let _ =
                                    tx.try_send(ShellEvent::OpenRenamePresetPrompt(name.clone()));
                            }
                        })
                        .child(codicon("\u{ebeb}", 16.0).text_color(rgba(p.text_muted)))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(name.clone()),
                        ),
                )
                .child(icon_btn(
                    format!("lpp-ow-{i}"),
                    "\u{eb4a}", // codicon-save-as
                    false,
                    "Overwrite with the current layout",
                    ShellEvent::OverwriteLayoutPreset(name.clone()),
                    tx.clone(),
                ))
                .child(icon_btn(
                    format!("lpp-exp-{i}"),
                    "\u{ea78}", // codicon-desktop-download
                    false,
                    "Export this layout to JSON",
                    ShellEvent::ExportPreset(name.clone()),
                    tx.clone(),
                ))
                .child(icon_btn(
                    format!("lpp-star-{i}"),
                    if is_default { "\u{eb59}" } else { "\u{ea6a}" },
                    is_default,
                    if is_default {
                        "Default on startup (click to unset)"
                    } else {
                        "Set as startup default"
                    },
                    ShellEvent::SetDefaultLayoutPreset(name.clone()),
                    tx.clone(),
                ))
                .child(icon_btn(
                    format!("lpp-del-{i}"),
                    "\u{ea81}", // codicon-trash
                    false,
                    "Delete layout",
                    ShellEvent::DeleteLayoutPreset(name.clone()),
                    tx.clone(),
                )),
        );
    }
    col
}
