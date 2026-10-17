//! Регистрация области попадания после prepaint дочернего элемента.

use gpui::{AnyElement, App, Bounds, Hitbox, HitboxBehavior, Pixels, Window};

pub(crate) fn prepaint_hitbox(
    child: &mut AnyElement,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Hitbox {
    child.prepaint(window, cx);
    window.insert_hitbox(bounds, HitboxBehavior::Normal)
}
