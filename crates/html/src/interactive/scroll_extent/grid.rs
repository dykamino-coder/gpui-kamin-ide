//! Measured grid track bounds contribute to scrollable overflow (CSS Grid §5.3).

use super::{Frame, exact_probe::sticky_shift};
use crate::hooks::{Hooks, IntoHooked};
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, App, Bounds, LayoutId, Pixels, Window, px};

pub(super) fn probe(frame: Frame, c: &Computed) -> Option<AnyElement> {
    matches!(
        c.display,
        Some(Display::Grid | Display::InlineGrid | Display::GridLanes)
    )
    .then(|| GridProbe(frame).hooked())
}

struct GridProbe(Frame);
impl Hooks for GridProbe {
    type Layout = LayoutId;
    type Prepaint = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, Self::Layout) {
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.inset.top = px(0.0).into();
        style.inset.left = px(0.0).into();
        let id = window.request_layout(style, [], cx);
        (id, id)
    }

    fn prepaint(
        &mut self,
        _: Bounds<Pixels>,
        id: &mut Self::Layout,
        window: &mut Window,
        _: &mut App,
    ) -> Self::Prepaint {
        let Some((origin, cols, rows)) = window.parent_grid_tracks(*id) else {
            return;
        };
        let origin = origin - sticky_shift();
        let span = |tracks: &[(f32, f32)], origin: Pixels| {
            let start = tracks.first().map_or(0.0, |t| t.0);
            let end = tracks.last().map_or(start, |t| t.1);
            (f32::from(origin) + start, f32::from(origin) + end)
        };
        let (left, right) = span(&cols, origin.x);
        let (top, bottom) = span(&rows, origin.y);
        self.0.grid_bounds.set(Some([left, top, right, bottom]));
    }

    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut Self::Layout,
        _: &mut Self::Prepaint,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}
