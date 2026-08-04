//! Контекст-меню сессии, файла, таба и поповеры оформления.
//!
//! Слой вынесен из `OverlayWindow::render` как есть (`plan/100-refactor-250.md`).

use crate::host_link::ShellEvent;
use crate::overlay::tool_menu::tool_tab_menu;
use crate::root::RootView;
use gpui::prelude::*;

use gpui::Div;

pub(crate) fn add_menus_context(
    mut layer: Div,
    r: &RootView,
    p: &'static kamin_theme::Palette,
    tx: &smol::channel::Sender<ShellEvent>,
    vw: f32,
    vh: f32,
    window: &mut gpui::Window,
) -> Div {
    if let Some(menu) = r.web_menu.as_ref() {
        layer = layer.child(crate::ui::web_menu::web_menu(menu, tx, vw, vh, p));
    }

    if let Some(menu) = r.session_menu.clone() {
        // Hit-регион кладёт само меню (hit_area в его корне): вне него
        // WM_NCHITTEST отдаёт ввод в main (скрим-закрытие делает main,
        // ховеры соседних элементов живут).
        layer = layer.child(crate::ui::context_menu::session_menu(&menu, tx, vw, vh, p));
    }

    if let Some(menu) = r.file_menu.clone() {
        layer = layer.child(crate::ui::file_menu::file_menu(
            &menu,
            &r.explorer_menu,
            tx,
            vw,
            vh,
            p,
        ));
    }

    if let Some(menu) = r.ed.editor_tab_menu.as_ref() {
        layer = layer.child(crate::ui::editor_tabs::editor_tab_menu(menu, tx, vw, vh, p));
    }

    if r.layout_popover {
        layer = layer.child(crate::ui::layout_popover::layout_popover(
            &r.layout, vw, vh, tx, p,
        ));
    }
    if r.appearance_popover {
        layer = layer.child(crate::ui::layout_popover::appearance_popover(
            r.theme,
            r.theme_choice,
            &r.contrib_themes,
            r.contrib_theme_id.as_deref(),
            &r.icon_themes,
            r.icon_theme_id.as_deref(),
            vw,
            window,
            tx,
            p,
        ));
    }

    if let Some((slot, id, x, y)) = r.tool_tab_menu.clone() {
        layer = layer.child(tool_tab_menu(
            slot,
            id,
            x,
            y,
            r.tool_menu_sub,
            vw,
            vh,
            tx,
            p,
        ));
    }
    layer
}
