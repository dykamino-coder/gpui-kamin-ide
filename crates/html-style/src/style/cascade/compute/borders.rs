//! CSS Backgrounds 3 computed border widths use the CSS Values 4 device grid.

use crate::style::computed::Computed;
use crate::style::values::value::Len;

/// Preserve integral device pixels, promote positive hairlines, truncate the rest.
fn snap(width: f32, scale: f32) -> f32 {
    if width <= 0.0 {
        return 0.0;
    }
    (width * scale).floor().max(1.0) / scale
}

pub(super) fn resolve(style: &mut Computed, scale: f32) {
    let before = style.reset_border.border_width;
    let mut after = before;
    for side in [
        &mut after.top,
        &mut after.right,
        &mut after.bottom,
        &mut after.left,
    ] {
        if let Some(Len::Px(width)) = *side {
            *side = Some(Len::Px(snap(width, scale)));
        }
    }
    // Borderless elements and unchanged groups retain their shared allocation.
    if before != after {
        style.reset_border.border_width = after;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_device_pixels_including_hairlines_and_zero() {
        for (width, scale, expected) in [
            (0.0, 1.25, 0.0),
            (0.1, 1.25, 0.8),
            (0.8, 1.25, 0.8),
            (1.0, 1.25, 0.8),
            (2.0, 1.25, 1.6),
            (2.4, 1.25, 2.4),
            (1.0, 2.0, 1.0),
            (1.9, 1.0, 1.0),
        ] {
            assert!((snap(width, scale) - expected).abs() < 0.00001);
        }
    }

    #[test]
    fn an_integral_or_absent_border_keeps_its_shared_group() {
        let mut original = Computed::default();
        original.reset_border.border_width.top = Some(Len::Px(0.8));
        let mut changed = original.clone();
        resolve(&mut changed, 1.25);
        assert!(original.reset_border.ptr_eq(&changed.reset_border));
        resolve(&mut changed, 2.0);
        assert_eq!(changed.reset_border.border_width.top, Some(Len::Px(0.5)));
        assert_eq!(original.reset_border.border_width.top, Some(Len::Px(0.8)));
    }
}
