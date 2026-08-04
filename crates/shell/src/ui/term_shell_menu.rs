//! Дропдаун профилей у кнопки «+» терминала (PowerShell / cmd / Git Bash).
//!
//! Вынесено из `term_toolbar.rs` без изменения поведения
//! (`plan/100-refactor-250.md`).

use crate::host::events::TermEvent;
use gpui::prelude::*;
use gpui::{SharedString, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

use crate::colors::{rgba, tint};
use crate::host_link::ShellEvent;
use crate::ui::term_tb_parts::aw_fallback;

/// Минимальная ширина меню — от неё считается кламп по вьюпорту.
const MENU_MIN_W: f32 = 200.0;

/// Коробка меню шеллов, спозиционированная относительно кнопки «+».
pub(crate) fn shell_menu(
    default_shell: Option<&str>,
    viewport_w: f32,
    viewport_h: f32,
    p: &'static Palette,
    tx: &Sender<ShellEvent>,
) -> gpui::Stateful<gpui::Div> {
    let menu_left_rel = match crate::probe::registry::bounds_of("term-add-btn") {
        Some([ax, _, aw, _]) => {
            let want = ax + aw / 2.0 - MENU_MIN_W / 2.0;
            let clamped = want.clamp(8.0, (viewport_w - MENU_MIN_W - 8.0).max(8.0));
            clamped - ax
        }
        None => aw_fallback(MENU_MIN_W),
    };
    let mut menu = div()
        .id("term-shell-menu")
        .occlude()
        .absolute()
        // `POPUP_OFFSET_PX = 6` от нижней кромки кнопки 28 в обёртке 30
        .top(px(29.0 + 6.0))
        // `left = a.left + a.width/2 − p.width/2` + кламп гуттером 8
        // (`clamp-popup.ts:99-112`), пересчитанный в координаты якоря;
        // до первого замера — прежний правый край (ревью ц.19)
        .left(px(menu_left_rel))
        .min_w(px(MENU_MIN_W))
        .flex()
        .flex_col()
        .gap(px(1.0))
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .shadow(crate::ui::shadows::dropdown())
        // `max-height: calc(100vh - 16px); overflow-y: auto`
        .max_h(px((viewport_h - 16.0).max(80.0)))
        .overflow_y_scroll()
        .font_family(crate::root::UI_FONT);
    if crate::term::profiles().is_empty() {
        // `.menuEmpty` — 8/12, fs-sm, text-muted
        menu = menu.child(
            div()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_2))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_muted))
                .child("No shells discovered"),
        );
    }
    for prof in crate::term::profiles() {
        let is_default = default_shell == Some(prof.id.as_str());
        // `.menuRow { display: flex; gap: 2 }`
        menu = menu.child(
            div()
                .flex()
                .items_center()
                .gap(px(2.0))
                .child(profile_row(prof, is_default, tx, p))
                .child(default_star(prof, is_default, tx, p)),
        );
    }
    menu
}

/// `.menuItem` — отдельная кнопка внутри строки: `flex: 1`, padding 8/12,
/// ховер ТОЛЬКО на ней; звезда живёт снаружи (ревью ц.16: у нас она была
/// внутри и уезжала на 17.6 от края).
fn profile_row(
    prof: &crate::term::ShellProfile,
    is_default: bool,
    tx: &Sender<ShellEvent>,
    p: &'static Palette,
) -> gpui::Stateful<gpui::Div> {
    let hover_bg = tint(rgba(p.text_primary), 0.10);
    let open_tx = tx.clone();
    let id = prof.id.clone();
    let mut row = div()
        .id(SharedString::from(format!("term-prof-{}", prof.id)))
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
        .hover(move |s| s.bg(hover_bg))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            let _ = open_tx.try_send(ShellEvent::Term(TermEvent::TermNew(id.clone())));
        })
        .child(
            div()
                .w(px(16.0))
                .flex()
                .justify_center()
                .text_color(rgba(p.text_muted))
                // `.itemIcon` кегля не задаёт → база codicon 16.
                // Глиф — ПО ПРОФИЛЮ (`s.icon` оригинала:
                // terminal-powershell / -cmd / -bash / -linux), а не
                // общий codicon-terminal на всех
                .child(crate::ui::icon::codicon_str(
                    crate::ui::codicon_map::codicon_by_name(prof.icon).unwrap_or("\u{ea85}"),
                    16.0,
                )),
        )
        .child(div().flex_1().whitespace_nowrap().child(prof.label.clone()));
    if is_default {
        // .defaultTag: fs-xs, text-muted, uppercase, ls .04em
        row = row.child(
            div()
                .text_size(px(m::FS_XS))
                .letter_spacing(px(m::FS_XS * 0.04))
                .text_color(rgba(p.text_muted))
                .child("DEFAULT"),
        );
    }
    row
}

/// Звезда «шелл по умолчанию» справа от строки профиля.
fn default_star(
    prof: &crate::term::ShellProfile,
    is_default: bool,
    tx: &Sender<ShellEvent>,
    p: &'static Palette,
) -> gpui::Stateful<gpui::Div> {
    let star_tx = tx.clone();
    let id = prof.id.clone();
    div()
        .id(SharedString::from(format!("term-star-{}", prof.id)))
        .w(px(24.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(m::RADIUS_SM))
        // .starOn: accent-primary (не yellow)
        .text_color(if is_default {
            rgba(p.accent_primary)
        } else {
            rgba(p.text_muted)
        })
        .hover({
            let hb = tint(rgba(p.text_primary), 0.10);
            move |s| {
                if is_default {
                    s.bg(hb)
                } else {
                    s.bg(hb).text_color(rgba(p.text_primary))
                }
            }
        })
        .tooltip(crate::ui::tooltip::tooltip(if is_default {
            "Default shell"
        } else {
            "Set as default"
        }))
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            let _ = star_tx.try_send(ShellEvent::Term(TermEvent::TermSetDefaultShell(id.clone())));
        })
        // codicon star-full / star (empty-вариант в шрифте общий)
        .child(crate::ui::icon::codicon(
            if is_default { "\u{eb59}" } else { "\u{ea6a}" },
            12.0,
        ))
}
