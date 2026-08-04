//! Таб терминала в тулбаре: заголовок, активность, закрытие.
//!
//! Тело цикла вынесено как есть (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::TermEvent;
use crate::host_link::ShellEvent;
use crate::ui::term_tb_parts::concave_corner;
use gpui::prelude::*;
use gpui::{SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

pub(crate) fn term_tab<E: gpui::ParentElement>(
    tabs: E,
    i: usize,
    t: &crate::term::TermSession,
    active: usize,
    tx: &Sender<ShellEvent>,
    p: &Palette,
) -> E {
    let mut tabs = tabs;
    let is_active = i == active;
    // .tab: h30, px10, скругление только сверху 8, fs 11 / 500 / .02em
    let hover_bg = tint(rgba(p.bg_surface), 0.5);
    let group = SharedString::from(format!("term-tab-g-{i}"));
    let mut tab = div()
        .id(SharedString::from(format!("term-tab-{i}")))
        .group(group.clone())
        .relative()
        .flex()
        .items_center()
        .gap(px(6.0))
        .h(px(30.0))
        .px(px(10.0))
        // `.tab { flex: 0 1 auto }` — при тесноте табы ужимаются
        .flex_shrink()
        .min_w(px(80.0))
        .max_w(px(220.0))
        .rounded_tl(px(8.0))
        .rounded_tr(px(8.0))
        .text_size(px(11.0))
        .letter_spacing(px(11.0 * 0.02))
        .font_weight(gpui::FontWeight::MEDIUM)
        .font_family(crate::root::UI_FONT)
        .text_color(rgba(p.text_secondary))
        .whitespace_nowrap()
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, {
            let tx = tx.clone();
            move |_, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Term(TermEvent::TermSelect(i)));
            }
        })
        .child(crate::ui::icon::codicon("\u{ea85}", 12.0)) // codicon-terminal
        .child(
            div()
                .max_w(px(160.0))
                .overflow_hidden()
                .text_ellipsis()
                .child(SharedString::from(t.title.clone())),
        );
    if is_active {
        // Активный сливается с editor-bg поверхностью тела +
        // вогнутые уголки 6×6 по бокам (приём Chrome/JetBrains)
        tab = tab
            .bg(rgba(p.editor_bg))
            .text_color(rgba(p.text_primary))
            .child(concave_corner(rgba(p.editor_bg), true))
            .child(concave_corner(rgba(p.editor_bg), false));
    } else {
        tab = tab.hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)));
    }
    // Close ×: скрыт (opacity 0), виден на hover таба и на активном
    let txc = tx.clone();
    let mut close = div()
        .id(SharedString::from(format!("term-tabx-{i}")))
        .w(px(16.0))
        .h(px(16.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_XS))
        .hover({
            let hb = tint(rgba(p.bg_overlay), 0.6);
            move |s| s.bg(hb).opacity(1.0)
        })
        .tooltip(crate::ui::tooltip::tooltip("Close"))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            let _ = txc.try_send(ShellEvent::Term(TermEvent::TermClose(i)));
        })
        .child(crate::ui::icon::codicon("\u{ea76}", 11.0));
    close = if is_active {
        close.opacity(0.7)
    } else {
        close.opacity(0.0).group_hover(group, |s| s.opacity(0.7))
    };
    tab = tab.child(close);
    tabs = tabs.child(tab);
    tabs
}
