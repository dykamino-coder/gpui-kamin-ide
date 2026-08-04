//! Строки панели Problems: заголовок файла и одна диагностика.
//!
//! Вынесено из `problems.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
use crate::host::events::EdEvent;
use gpui::prelude::*;
use gpui::{SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::icon::codicon;
use crate::ui::problems_diag::Diag;
use crate::ui::problems_parts::dir_name;

/// `.fileRow`: h24 gap6 px8, chevron 13/w16, TreeIcon 16, имя primary,
/// dirname flex1 muted fs-xs (+tooltip uri), пилюля-счётчик.
pub(crate) fn file_row(
    uri: &str,
    name: &str,
    count: usize,
    is_collapsed: bool,
    tx: &Sender<ShellEvent>,
    p: &'static Palette,
) -> gpui::Stateful<gpui::Div> {
    let tx = tx.clone();
    let uri_owned = uri.to_string();
    div()
        .id(SharedString::from(format!("prob-file-{uri}")))
        .flex()
        .items_center()
        .gap(px(6.0))
        .w_full()
        .h(px(24.0))
        .px(px(m::SPACE_2))
        .text_color(rgba(p.text_secondary))
        .whitespace_nowrap()
        .overflow_hidden()
        .cursor_pointer()
        .hover({
            let hb = tint(rgba(p.bg_surface), 0.6);
            move |s| s.bg(hb)
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::ToggleProblemsFile(
                uri_owned.clone(),
            )));
        })
        .child(
            div()
                .w(px(16.0))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .text_color(rgba(p.text_muted))
                // `.chevron{font-size:13px}` стоит на самом
                // `<i class="codicon …">` → проигрывает вендорной базе
                // (ревью ц.14)
                .child(codicon(
                    if is_collapsed { "\u{eab6}" } else { "\u{eab4}" },
                    16.0,
                )),
        )
        .child(
            crate::icon_theme::file_img(name)
                .flex_shrink_0()
                .w(px(16.0))
                .h(px(16.0)),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_color(rgba(p.text_primary))
                .child(name.to_string()),
        )
        .child(
            div()
                .id(SharedString::from(format!("prob-dir-{uri}")))
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .tooltip(crate::ui::tooltip::tooltip(uri.to_string()))
                .child(dir_name(uri)),
        )
        .child(
            // .fileCount: пилюля min-w16 h16 px5 r9 bg-surface
            div()
                .flex_shrink_0()
                .min_w(px(16.0))
                .h(px(16.0))
                .px(px(5.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(9.0))
                .bg(rgba(p.bg_surface))
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .child(format!("{count}")),
        )
}

/// `.row`: min-h22, pl26/pr8, gap6, hover surface 60 % + text-primary.
pub(crate) fn diag_row(
    uri: &str,
    idx: usize,
    d: &Diag,
    tx: &Sender<ShellEvent>,
    p: &'static Palette,
) -> gpui::Stateful<gpui::Div> {
    let (glyph, color) = match d.severity {
        0 => ("\u{ea87}", rgba(p.accent_red)),    // error
        1 => ("\u{ea6c}", rgba(p.accent_yellow)), // warning
        2 => ("\u{ea74}", rgba(p.accent_blue)),   // info
        _ => ("\u{ea61}", rgba(p.text_muted)),    // lightbulb (hint)
    };
    let tx = tx.clone();
    let uri_owned = uri.to_string();
    let line = d.line;
    let origin = d.origin();
    div()
        .id(SharedString::from(format!("prob-{uri}-{idx}")))
        .flex()
        .items_center()
        .gap(px(6.0))
        .w_full()
        .min_h(px(22.0))
        .pl(px(26.0))
        .pr(px(m::SPACE_2))
        .whitespace_nowrap()
        .overflow_hidden()
        .text_color(rgba(p.text_secondary))
        .cursor_pointer()
        .hover({
            let hb = tint(rgba(p.bg_surface), 0.6);
            move |s| s.bg(hb).text_color(rgba(p.text_primary))
        })
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, _| {
            let _ = tx.try_send(ShellEvent::Ed(EdEvent::OpenFileAt(
                uri_owned.clone(),
                line + 1,
            )));
        })
        // `.sevIcon { font-size: 14px }` (0,1,0) проигрывает базе
        // codicon (0,2,0) → 16 (ревью ц.13)
        .child(codicon(glyph, 16.0).flex_shrink_0().text_color(color))
        .child(
            div()
                .id(SharedString::from(format!("prob-msg-{uri}-{idx}")))
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .tooltip(crate::ui::tooltip::tooltip(d.message.clone()))
                .child(d.message.clone()),
        )
        .when(!origin.is_empty(), |el| {
            el.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(m::FS_XS))
                    .text_color(rgba(p.text_muted))
                    .child(origin),
            )
        })
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(m::FS_XS))
                .text_color(rgba(p.text_muted))
                .child(format!("[Ln {}, Col {}]", d.line + 1, d.character + 1)),
        )
}
