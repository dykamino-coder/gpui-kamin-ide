//! Фаза `request_layout` элемента `ClampCut`: абсолютная позиция и размер 100%×100%, чтобы
//! границы элемента совпадали со всем clamp-контейнером и точка среза мерилась от его верха.

use super::ClampCut;
use gpui::{App, LayoutId, Window};

impl ClampCut {
    pub(crate) fn request_layout_impl(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = gpui::Style {
            position: gpui::Position::Absolute,
            ..Default::default()
        };
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }
}
