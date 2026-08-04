//! Extensions-панель (ExtensionsPanel 1:1): хедер «EXTENSIONS» + Install,
//! группы «Installed — N» / «Built-in — N», строка: иконка 26 (fallback
//! codicon-extensions) + имя + «version · status» + Enable/Disable-кнопка +
//! uninstall (не builtin). Disabled-строка приглушена 0.55.
//! (Реальные иконки расширений — data-URL с хоста — фаза расширений.)

pub use crate::ui::ext_row::ExtDesc;

use crate::host::events::CzEvent;
use crate::ui::ext_row::{ext_row, group_header};
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use gpui_component::scroll::ScrollableElement as _;
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;

/// Тело Extensions-панели. None = загрузка.
pub fn extensions_panel(
    exts: Option<&Vec<ExtDesc>>,
    icons: &std::collections::HashMap<String, Option<String>>,
    status: &str,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> AnyElement {
    let Some(exts) = exts else {
        return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .flex_col()
            .gap(px(m::SPACE_2))
            .text_color(rgba(p.text_muted))
            .text_size(px(m::FS_SM))
            // Вместо безмолвного «Loading…» — что именно делает загрузчик
            .child(status.to_string())
            .into_any_element();
    };
    let mut sorted: Vec<&ExtDesc> = exts.iter().collect();
    // `localeCompare` регистронезависим — байтовый `cmp` ставил «Zebra»
    // перед «apple» (ревью ц.13)
    sorted.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
    let installed: Vec<&ExtDesc> = sorted.iter().copied().filter(|e| !e.builtin).collect();
    let builtin: Vec<&ExtDesc> = sorted.iter().copied().filter(|e| e.builtin).collect();

    // .header: «EXTENSIONS» + .installBtn (accent 14% + бордер 40%)
    let header = {
        let tx = tx.clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(m::SPACE_2))
            .flex_shrink_0()
            .pl(px(m::SPACE_3))
            .pr(px(m::SPACE_2))
            .py(px(m::SPACE_1))
            .text_size(px(m::FS_XS))
            .letter_spacing(px(m::FS_XS * 0.04))
            .text_color(rgba(p.text_muted))
            .child("EXTENSIONS")
            .child(
                div()
                    .id("ext-install-vsix")
                    // `.installBtn { letter-spacing: 0 }`
                    .letter_spacing(px(0.))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .px(px(m::SPACE_2))
                    .py(px(3.0))
                    .rounded(px(m::RADIUS_SM))
                    .border_1()
                    .border_color(tint(rgba(p.accent_primary), 0.4))
                    .bg(tint(rgba(p.accent_primary), 0.14))
                    .text_color(rgba(p.text_primary))
                    .cursor_pointer()
                    .hover({
                        let hb = tint(rgba(p.accent_primary), 0.26);
                        move |s| s.bg(hb)
                    })
                    .tooltip(crate::ui::tooltip::tooltip("Install from a .vsix archive"))
                    .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                        cx.stop_propagation();
                        let _ = tx.try_send(ShellEvent::Cz(CzEvent::InstallVsixPrompt));
                    })
                    .child(crate::ui::icon::codicon("\u{eac2}", 12.0)) // cloud-download
                    .child("Install"),
            )
    };

    // Регион досье: корень панели расширений
    let mut first_row = Some(());
    let mut list = div()
        .relative()
        .child(crate::probe::registry::probe_area("extensions-panel"))
        .id("extensions-list")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scrollbar()
        .px(px(m::SPACE_2))
        .pb(px(m::SPACE_2));
    if sorted.is_empty() {
        list = list.child(
            div()
                .p(px(m::SPACE_3))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_muted))
                .child("No extensions installed."),
        );
    }
    if !installed.is_empty() {
        list = list.child(group_header("Installed", installed.len(), p));
        for e in &installed {
            // Регион досье — на ПЕРВОЙ строке списка
            let row = ext_row(e, icons.get(&e.id), tx, p);
            list = list.child(if first_row.take().is_some() {
                div()
                    .relative()
                    .child(crate::probe::registry::probe_area("extension-row"))
                    .child(row)
                    .into_any_element()
            } else {
                row
            });
        }
    }
    if !builtin.is_empty() {
        list = list.child(group_header("Built-in", builtin.len(), p));
        for e in &builtin {
            list = list.child(ext_row(e, icons.get(&e.id), tx, p));
        }
    }

    div()
        .id("extensions-panel")
        .flex()
        .flex_col()
        .size_full()
        .min_h(px(0.))
        .child(header)
        .child(list)
        .into_any_element()
}
