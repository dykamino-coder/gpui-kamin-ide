//! Контекст-меню таба редактора.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::colors::{rgba, tint};
use crate::host::events::EdEvent;
use crate::host_link::ShellEvent;
use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_metrics as m;
use kamin_theme::Palette;
use smol::channel::Sender;

/// Контекст-меню файл-таба (в overlay): Close-группа + переход к файловым
/// действиям (Reveal-类 действия — из меню дерева по path).
pub struct EditorTabMenu {
    pub index: usize,
    pub path: String,
    pub x: f32,
    pub y: f32,
    pub pinned: bool,
}
pub fn editor_tab_menu(
    menu: &EditorTabMenu,
    tx: &Sender<ShellEvent>,
    viewport_w: f32,
    viewport_h: f32,
    p: &Palette,
) -> AnyElement {
    const MENU_W: f32 = 220.0;
    const MARGIN: f32 = 8.0;
    let est_h = 190.0;
    let x = menu.x.min(viewport_w - MENU_W - MARGIN).max(MARGIN);
    let y = menu.y.min(viewport_h - est_h - MARGIN).max(MARGIN);
    let i = menu.index;

    let item = |id: &'static str,
                label: &'static str,
                tx: Sender<ShellEvent>,
                ev: fn(usize) -> EdEvent| {
        let hover_bg = tint(rgba(p.text_primary), 0.08);
        div()
            .id(id)
            .flex()
            .items_center()
            .px(px(m::SPACE_3))
            .py(px(m::SPACE_1))
            .rounded(px(m::RADIUS_SM))
            .text_size(px(m::FS_SM))
            .text_color(rgba(p.text_secondary))
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
                let _ = tx.try_send(ShellEvent::Ed(ev(i)));
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseEditorTabMenu));
            })
            .child(label)
            .into_any_element()
    };

    let path = menu.path.clone();
    let hover_bg = tint(rgba(p.text_primary), 0.08);
    div()
        .id("editor-tab-menu")
        .occlude()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(MENU_W))
        .flex()
        .flex_col()
        .p(px(m::SPACE_1))
        .rounded(px(m::RADIUS_MD))
        .bg(rgba(p.bg_surface))
        .border_1()
        .border_color(tint(rgba(p.text_primary), 0.06))
        .child(crate::overlay::hit_area())
        .child(item(
            "etm-pin",
            if menu.pinned { "Unpin Tab" } else { "Pin Tab" },
            tx.clone(),
            EdEvent::TogglePinEditorTab,
        ))
        .child(item(
            "etm-close",
            "Close",
            tx.clone(),
            EdEvent::CloseEditorTab,
        ))
        .child(item(
            "etm-others",
            "Close Others",
            tx.clone(),
            EdEvent::CloseOtherEditorTabs,
        ))
        .child(item(
            "etm-right",
            "Close to the Right",
            tx.clone(),
            EdEvent::CloseEditorTabsRight,
        ))
        .child(item("etm-all", "Close All", tx.clone(), |_| {
            EdEvent::CloseAllEditorTabs
        }))
        .child(
            div()
                .h(px(1.0))
                .mx(px(m::SPACE_2))
                .my(px(3.0))
                .bg(tint(rgba(p.text_primary), 0.06)),
        )
        .child(
            // Файловые действия дерева по этому path (Copy Path и т.д.)
            div()
                .id("etm-file")
                .flex()
                .items_center()
                .px(px(m::SPACE_3))
                .py(px(m::SPACE_1))
                .rounded(px(m::RADIUS_SM))
                .text_size(px(m::FS_SM))
                .text_color(rgba(p.text_secondary))
                .cursor_pointer()
                .hover(move |s| s.bg(hover_bg).text_color(rgba(p.text_primary)))
                .on_mouse_down(gpui::MouseButton::Left, {
                    let tx = tx.clone();
                    let (mx, my) = (x, y);
                    move |_, _, cx| {
                        cx.stop_propagation();
                        let _ = tx.try_send(ShellEvent::Ed(EdEvent::CloseEditorTabMenu));
                        let _ = tx.try_send(ShellEvent::Ed(EdEvent::OpenFileMenu(
                            path.clone(),
                            false,
                            mx,
                            my,
                        )));
                    }
                })
                .child("File actions…"),
        )
        .into_any_element()
}
