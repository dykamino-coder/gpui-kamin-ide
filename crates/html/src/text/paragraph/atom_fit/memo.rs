//! Intrinsic measurements of one atomic inline's subtree (`AtomMemo`), kept on
//! the atom's layout root while the frame tree keeps that subtree.

use crate::tolerance::COORD_EPS;
use gpui::{App, Window, size};
use std::cell::Cell;

/// What the atom's subtree alone decides, kept on its layout root while the frame
/// tree keeps the subtree (`gpui::Window::set_layout_memo`): it is not measured again.
pub(super) struct AtomMemo {
    /// Margin-box min-content and max-content widths.
    pub(super) min: f32,
    pub(super) max: f32,
    /// Max-content height and baseline; `None` if min-content is max-content.
    pub(super) snap: Option<(f32, Option<f32>)>,
    /// Copy measured at a width: (width, height, baseline).
    pub(super) at_width: Cell<Vec<(f32, f32, Option<f32>)>>,
}

impl AtomMemo {
    /// Measure the copy `snap` of an atom wrapper laid out at max-content.
    pub(super) fn measure(
        snap: &mut gpui::LayoutMeasurement,
        max: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let content = |w| size(w, gpui::AvailableSpace::MaxContent);
        let (smin, _, _) = snap.measure(content(gpui::AvailableSpace::MinContent), window, cx);
        let min = f32::from(smin.width);
        let snap = (min < max - COORD_EPS).then(|| {
            let (smax, sbase, _) =
                snap.measure(content(gpui::AvailableSpace::MaxContent), window, cx);
            (f32::from(smax.height), sbase.map(f32::from))
        });
        Self {
            min,
            max,
            snap,
            at_width: Cell::default(),
        }
    }
}
