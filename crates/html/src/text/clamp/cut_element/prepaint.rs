//! Фаза `prepaint` элемента `ClampCut`: пустая. Пробы строк пишут свои границы в prepaint
//! своего кадра, а срез считается уже в `paint`, когда все пробы контейнера собраны.

use super::ClampCut;
use gpui::{App, Bounds, Pixels, Window};

impl ClampCut {
    pub(crate) fn prepaint_impl(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}
