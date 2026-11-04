//! Регистрация области попадания после prepaint дочернего элемента.

use gpui::{AnyElement, App, Bounds, Hitbox, HitboxBehavior, Pixels, Window};

pub(crate) async fn prepaint_hitbox_async(
    child: &mut AnyElement,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
    stack: &mut gpui::PaintStack,
) -> Hitbox {
    child.prepaint_async(window, cx, stack).await;
    window.insert_hitbox(bounds, HitboxBehavior::Normal)
}
