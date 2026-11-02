//! Природный размер `<canvas>` и внутренние ключевые слова размеров
//! (css-sizing-3 §5.1, css-sizing-4 §4).
//! Вход — стиль холста после `apply_presentational_size`; выход — тот же
//! стиль, где `min-content`/`max-content`/`fit-content` стали точками, а при
//! обоих атрибутах и `aspect-ratio` — заданы `width`/`height`. Природная
//! высота выводится из природной ширины через соотношение ДО авторских
//! пределов (Blink `ComputeNormalizedNaturalSize`); `border-box` добавляет
//! края. Обособление по размеру отменяет всё это.

use crate::defaults::DEFAULT_CANVAS_SIZE_PX;
use crate::style::values::value::px_strict;
use crate::style::{computed::Computed, values::value::Len};

pub(super) fn resolve(s: &mut Computed) {
    if s.contains_width() || s.contains_height() {
        return;
    }
    let px = |v| px_strict(v);
    let intrinsic = |v| matches!(v, Some(Len::MinContent | Len::MaxContent | Len::FitContent));
    let ratio = s
        .reset_shapes
        .aspect_ratio
        .filter(|r| r.is_finite() && *r > 0.0);
    let edges = s.borders();
    let sum = |a, b, c, d| {
        px(a).unwrap_or(0.0) + px(b).unwrap_or(0.0) + px(c).unwrap_or(0.0) + px(d).unwrap_or(0.0)
    };
    let (bx, by) = if s.reset_border.border_box == Some(true) {
        (
            sum(
                s.reset_box_layout.padding.left,
                s.reset_box_layout.padding.right,
                edges.left,
                edges.right,
            ),
            sum(
                s.reset_box_layout.padding.top,
                s.reset_box_layout.padding.bottom,
                edges.top,
                edges.bottom,
            ),
        )
    } else {
        (0.0, 0.0)
    };
    let w = px(s.reset_box_layout.attr_width).unwrap_or(DEFAULT_CANVAS_SIZE_PX.0);
    // Blink ComputeNormalizedNaturalSize coerces the natural block size
    // from the natural inline size, before applying author constraints.
    let h = ratio.map_or_else(
        || px(s.reset_box_layout.attr_height).unwrap_or(DEFAULT_CANVAS_SIZE_PX.1),
        |r| ((w + bx) / r - by).max(0.0),
    );
    if s.reset_box_layout.attr_sized.0 && s.reset_box_layout.attr_sized.1 && ratio.is_some() {
        s.reset_box_layout.width = Some(Len::Px(w + bx));
        s.reset_box_layout.height = Some(Len::Px(h + by));
    }
    let transfer_y = |width: f32| ratio.map_or(h + by, |r| width / r);
    let intrinsic_h = if !s.reset_box_layout.attr_sized.0 {
        px(s.reset_box_layout.width).map_or(h + by, transfer_y)
    } else {
        h + by
    };
    for group_index in 0..3 {
        let v = match group_index {
            0 => &mut s.reset_box_layout.height,
            1 => &mut s.reset_box_layout.min_height,
            _ => &mut s.reset_box_layout.max_height,
        };
        if intrinsic(*v) {
            *v = Some(Len::Px(intrinsic_h));
        }
    }
    let clamp = |v: f32, min, max| {
        v.min(px(max).unwrap_or(f32::INFINITY))
            .max(px(min).unwrap_or(0.0))
    };
    let intrinsic_w = if !s.reset_box_layout.attr_sized.1 {
        px(s.reset_box_layout.height).map_or(w + bx, |height| {
            ratio.map_or(w + bx, |r| {
                clamp(
                    height,
                    s.reset_box_layout.min_height,
                    s.reset_box_layout.max_height,
                ) * r
            })
        })
    } else {
        w + bx
    };
    for index in 0..3 {
        let v = match index {
            0 => &mut s.reset_box_layout.width,
            1 => &mut s.reset_box_layout.min_width,
            _ => &mut s.reset_box_layout.max_width,
        };
        if intrinsic(*v) && s.reset_context.fit_arg[index].is_none() {
            *v = Some(Len::Px(intrinsic_w));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_dimension_integer_prefix_and_defaults() {
        for (raw, expected) in [
            ("100px", 100.0),
            ("100%", 100.0),
            (" +12.5", 12.0),
            ("-0tail", 0.0),
            ("-1", 300.0),
            ("2147483648", 300.0),
        ] {
            let mut s = Computed::default();
            super::super::presentational_size::apply_presentational_size(
                &mut s,
                "canvas",
                &[("width".into(), raw.into())],
            );
            assert!(
                matches!(s.reset_box_layout.attr_width, Some(Len::Px(v)) if v == expected),
                "{raw}"
            );
        }
    }

    #[test]
    fn coerced_natural_size_uses_asymmetric_border_box_edges() {
        let mut s = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    attr_width: Some(Len::Px(40.0)),
                    attr_height: Some(Len::Px(10.0)),
                    attr_sized: (true, true),
                    ..Default::default()
                },
            ),
            reset_shapes: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetShapes {
                    aspect_ratio: Some(2.0),
                    ..Default::default()
                },
            ),
            reset_border: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBorder {
                    border_box: Some(true),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        s.reset_box_layout.padding.right = Some(Len::Px(60.0));
        s.reset_box_layout.padding.bottom = Some(Len::Px(20.0));
        resolve(&mut s);
        assert!(matches!(s.reset_box_layout.width, Some(Len::Px(100.0))));
        assert!(matches!(s.reset_box_layout.height, Some(Len::Px(50.0))));
    }

    #[test]
    fn intrinsic_block_limit_precedes_inline_ratio_transfer() {
        for (height, min, max) in [
            (0.0, Some(Len::MaxContent), None),
            (500.0, None, Some(Len::MaxContent)),
        ] {
            let mut s = Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        attr_width: Some(Len::Px(100.0)),
                        attr_height: Some(Len::Px(50.0)),
                        width: Some(Len::MaxContent),
                        height: Some(Len::Px(height)),
                        min_height: min,
                        max_height: max,
                        ..Default::default()
                    },
                ),
                reset_shapes: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetShapes {
                        aspect_ratio: Some(1.0),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            resolve(&mut s);
            assert!(matches!(s.reset_box_layout.width, Some(Len::Px(100.0))));
        }
    }
}
