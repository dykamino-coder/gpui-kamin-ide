//! Clip solid border-area image layers by the same outer-minus-inner geometry.
//! CSS Backgrounds 4 #background-clip; Blink box_painter_base.cc:1695-1715
//! clips area-filling border styles directly, ignoring border-color alpha.

use crate::style::computed::{BgClip, Computed};
use crate::style::values::value::Len;
use gpui::{Bounds, Pixels, Window, point, px};

pub(super) fn paint(
    c: &Computed,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    mut fill: impl FnMut(&Computed, &mut Window),
) {
    if c.bg_clip != Some(BgClip::BorderArea)
        || c.radius != Default::default()
        || c.radius_ell.is_some()
        || c.border_shape.is_some()
        || c.border_image.is_some()
        || c.border_side_styles
            .iter()
            .any(|s| !matches!(s, None | Some(3..=6) | Some(9)))
    {
        return fill(c, window);
    }
    let b = c.borders();
    let widths: Option<Vec<f32>> = [b.top, b.right, b.bottom, b.left]
        .into_iter()
        .map(|v| match v {
            None => Some(0.0),
            Some(Len::Px(v)) => Some(v),
            _ => None,
        })
        .collect();
    let Some(b) = widths else {
        return fill(c, window);
    };
    // Clip uses the existing painting grid, while tile positioning retains
    // ExactLayer's unrounded box. Mixing them discards the last device edge.
    let outer = Bounds::from_corners(
        point(bounds.left() - px(b[3]), bounds.top() - px(b[0])),
        point(bounds.right() + px(b[1]), bounds.bottom() + px(b[2])),
    );
    let outer = super::sampling::snapped_clip(outer, window);
    let inner = super::sampling::snapped_clip(bounds, window);
    let rects = [
        Bounds::from_corners(outer.origin, point(outer.right(), inner.top())),
        Bounds::from_corners(point(outer.left(), inner.bottom()), outer.bottom_right()),
        Bounds::from_corners(
            point(outer.left(), inner.top()),
            point(inner.left(), inner.bottom()),
        ),
        Bounds::from_corners(
            point(inner.right(), inner.top()),
            point(outer.right(), inner.bottom()),
        ),
    ];
    for rect in rects {
        if rect.size.width > px(0.0) && rect.size.height > px(0.0) {
            window.with_content_mask(Some(gpui::ContentMask { bounds: rect }), |w| fill(c, w));
        }
    }
}
