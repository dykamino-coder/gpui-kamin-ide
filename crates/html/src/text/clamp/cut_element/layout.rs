//! Layout for cut_element; split out to keep the owning module within 250 lines.

use super::ClampCut;
use gpui::{App, LayoutId, Window};

impl ClampCut {
    pub(crate) fn request_layout_impl(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }
}
