//! Debug preview calls the real loader builders with no session or Bridge token.

use gpui::prelude::*;
use gpui::{AnyElement, div, px};
use kamin_theme::Palette;

pub(crate) fn preview(p: &'static Palette) -> AnyElement {
    let color = crate::colors::rgba(p.accent_primary);
    div()
        .flex()
        .flex_col()
        .size_full()
        .gap(px(16.0))
        .child(
            div()
                .relative()
                .flex_1()
                .min_h(px(200.0))
                .child(super::chat_switch_skeleton::brand_loader(p, "Loading…")),
        )
        .child(super::icon::spinner("acceptance-spinner", 24.0, color))
        .child(super::icon::spinner_ring(
            "acceptance-ring",
            24.0,
            color,
            color,
        ))
        .into_any_element()
}
