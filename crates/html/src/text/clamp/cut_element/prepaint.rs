//! Prepaint for cut_element; split out to keep the owning module within 250 lines.

use super::ClampCut;
use gpui::{App, Bounds, GlobalElementId, InspectorElementId, Pixels, Window};

impl ClampCut {
    pub(crate) fn prepaint_impl(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}
