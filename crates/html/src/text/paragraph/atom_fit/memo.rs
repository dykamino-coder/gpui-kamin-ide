//! Замеры поддерева атома, общие для кадров (`AtomMemo`).

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
    /// Copy measured at a width: (width, height, baseline), the most recent
    /// `AT_WIDTH_KEPT` widths.
    pub(super) at_width: Cell<Vec<(f32, f32, Option<f32>)>>,
}

/// Widths kept per atom: a live resize asks a new width in every frame, so an
/// unbounded list would grow with every width the window passed through.
const AT_WIDTH_KEPT: usize = 8;

impl AtomMemo {
    /// Add a measurement to `seen`, dropping the oldest beyond `AT_WIDTH_KEPT`.
    pub(super) fn remember(seen: &mut Vec<(f32, f32, Option<f32>)>, m: (f32, f32, Option<f32>)) {
        if seen.len() >= AT_WIDTH_KEPT {
            seen.remove(0);
        }
        seen.push(m);
    }

    /// Measure the copy `snap` of an atom wrapper laid out at max-content.
    pub(super) async fn measure(
        snap: &mut gpui::LayoutMeasurement,
        max: f32,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Self {
        let content = |w| size(w, gpui::AvailableSpace::MaxContent);
        let (smin, _, _) = snap.measure_async(content(gpui::AvailableSpace::MinContent), window, cx, stack).await;
        let min = f32::from(smin.width);
        let snap = if min < max - COORD_EPS {
            let (smax, sbase, _) =
                snap.measure_async(content(gpui::AvailableSpace::MaxContent), window, cx, stack).await;
            Some((f32::from(smax.height), sbase.map(f32::from)))
        } else { None };
        Self {
            min,
            max,
            snap,
            at_width: Cell::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_seen_during_a_resize_stay_bounded() {
        let mut seen = Vec::new();
        for w in 0..100 {
            AtomMemo::remember(&mut seen, (w as f32, 1.0, None));
        }
        assert_eq!(seen.len(), AT_WIDTH_KEPT);
        assert_eq!(
            seen.first().map(|m| m.0),
            Some((100 - AT_WIDTH_KEPT) as f32)
        );
        assert_eq!(seen.last().map(|m| m.0), Some(99.0));
    }
}
