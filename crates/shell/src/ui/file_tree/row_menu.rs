//! Правый клик по строке дерева: открытие файлового меню.
//!
//! Блок перенесён как есть (`plan/100-refactor-250.md`).

use gpui::prelude::*;

pub(crate) fn with_menu(
    row: gpui::Stateful<gpui::Div>,
    path: &str,
    is_dir: bool,
    on_menu: &(impl Fn(String, bool, f32, f32) + Clone + 'static),
) -> gpui::Stateful<gpui::Div> {
    let mut row = row;
    row = row.on_mouse_down(gpui::MouseButton::Right, {
        let cb = on_menu.clone();
        let path = path.to_string();
        move |ev: &gpui::MouseDownEvent, _, cx| {
            cx.stop_propagation();
            cb(
                path.clone(),
                is_dir,
                f32::from(ev.position.x),
                f32::from(ev.position.y),
            );
        }
    });
    row
}
