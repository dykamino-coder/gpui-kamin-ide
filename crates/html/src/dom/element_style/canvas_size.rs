//! Coerced natural canvas sizes and intrinsic constraints (CSS Sizing 3/4).

use crate::style::{computed::Computed, values::value::Len};

pub(super) fn resolve(s: &mut Computed) {
    if s.contains_width() || s.contains_height() {
        return;
    }
    let px = |v| match v {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let intrinsic = |v| matches!(v, Some(Len::MinContent | Len::MaxContent | Len::FitContent));
    let ratio = s.aspect_ratio.filter(|r| r.is_finite() && *r > 0.0);
    let edges = s.borders();
    let sum = |a, b, c, d| {
        px(a).unwrap_or(0.0) + px(b).unwrap_or(0.0) + px(c).unwrap_or(0.0) + px(d).unwrap_or(0.0)
    };
    let (bx, by) = if s.border_box == Some(true) {
        (
            sum(s.padding.left, s.padding.right, edges.left, edges.right),
            sum(s.padding.top, s.padding.bottom, edges.top, edges.bottom),
        )
    } else {
        (0.0, 0.0)
    };
    let w = px(s.attr_width).unwrap_or(300.0);
    // Blink ComputeNormalizedNaturalSize coerces the natural block size
    // from the natural inline size, before applying author constraints.
    let h = ratio.map_or_else(
        || px(s.attr_height).unwrap_or(150.0),
        |r| ((w + bx) / r - by).max(0.0),
    );
    if s.attr_sized.0 && s.attr_sized.1 && ratio.is_some() {
        s.width = Some(Len::Px(w + bx));
        s.height = Some(Len::Px(h + by));
    }
    let transfer_y = |width: f32| ratio.map_or(h + by, |r| width / r);
    let intrinsic_h = if !s.attr_sized.0 {
        px(s.width).map_or(h + by, transfer_y)
    } else {
        h + by
    };
    for v in [&mut s.height, &mut s.min_height, &mut s.max_height] {
        if intrinsic(*v) {
            *v = Some(Len::Px(intrinsic_h));
        }
    }
    let clamp = |v: f32, min, max| {
        v.min(px(max).unwrap_or(f32::INFINITY))
            .max(px(min).unwrap_or(0.0))
    };
    let intrinsic_w = if !s.attr_sized.1 {
        px(s.height).map_or(w + bx, |height| {
            ratio.map_or(w + bx, |r| clamp(height, s.min_height, s.max_height) * r)
        })
    } else {
        w + bx
    };
    for (index, v) in [&mut s.width, &mut s.min_width, &mut s.max_width]
        .into_iter()
        .enumerate()
    {
        if intrinsic(*v) && s.fit_arg[index].is_none() {
            *v = Some(Len::Px(intrinsic_w));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coerced_natural_size_uses_asymmetric_border_box_edges() {
        let mut s = Computed {
            attr_width: Some(Len::Px(40.0)),
            attr_height: Some(Len::Px(10.0)),
            attr_sized: (true, true),
            aspect_ratio: Some(2.0),
            border_box: Some(true),
            ..Computed::default()
        };
        s.padding.right = Some(Len::Px(60.0));
        s.padding.bottom = Some(Len::Px(20.0));
        resolve(&mut s);
        assert!(matches!(s.width, Some(Len::Px(100.0))));
        assert!(matches!(s.height, Some(Len::Px(50.0))));
    }

    #[test]
    fn intrinsic_block_limit_precedes_inline_ratio_transfer() {
        for (height, min, max) in [
            (0.0, Some(Len::MaxContent), None),
            (500.0, None, Some(Len::MaxContent)),
        ] {
            let mut s = Computed {
                attr_width: Some(Len::Px(100.0)),
                attr_height: Some(Len::Px(50.0)),
                aspect_ratio: Some(1.0),
                width: Some(Len::MaxContent),
                height: Some(Len::Px(height)),
                min_height: min,
                max_height: max,
                ..Computed::default()
            };
            resolve(&mut s);
            assert!(matches!(s.width, Some(Len::Px(100.0))));
        }
    }
}
