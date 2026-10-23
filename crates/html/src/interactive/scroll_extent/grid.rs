//! Щуп сеточных дорожек: измеренные границы колонок и рядов сеточного контейнера входят в
//! прокручиваемое переполнение (CSS Grid §5.3), даже когда элементы сетки их не заполняют.

use super::{
    Frame,
    exact_probe::{StickyShift, sticky_shift},
};
use crate::style::computed::{Computed, Display};
use gpui::{AnyElement, Pixels};

pub(super) fn probe(frame: Frame, c: &Computed, sticky: Option<StickyShift>) -> Option<AnyElement> {
    if !matches!(
        c.display,
        Some(Display::Grid | Display::InlineGrid | Display::GridLanes)
    ) {
        return None;
    }
    let mut style = crate::layout::probe::full_box();
    style.size = Default::default();
    Some(frame.grid_bounds.record_styled(style, move |g, window| {
        let (origin, cols, rows) = window.parent_grid_tracks(g.id)?;
        let origin = origin - sticky_shift(sticky.as_ref());
        let span = |tracks: &[(f32, f32)], origin: Pixels| {
            let start = tracks.first().map_or(0.0, |t| t.0);
            let end = tracks.last().map_or(start, |t| t.1);
            (f32::from(origin) + start, f32::from(origin) + end)
        };
        let (left, right) = span(&cols, origin.x);
        let (top, bottom) = span(&rows, origin.y);
        Some([left, top, right, bottom])
    }))
}
