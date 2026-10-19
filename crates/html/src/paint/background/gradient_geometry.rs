//! CSS Images 3 gradient lines and radial ending shapes in concrete image coordinates.
//! Blink CSSRadialGradientValue::CreateGradient uses side distances first, then
//! expands an ellipse of that aspect ratio through the selected corner.

use super::len_px;
use crate::document::state::ValueCx;
use crate::style::{computed::parse_pos_words, values::value::Len};

pub(super) enum Geometry {
    Linear {
        direction: (f32, f32),
        length: f32,
    },
    Radial {
        center: (f32, f32),
        radii: (f32, f32),
    },
}

impl Geometry {
    pub(super) fn new(raw: &str, size: (f32, f32), angle: f32, vcx: ValueCx<'_>) -> Option<Self> {
        let raw = raw.strip_prefix("repeating-").unwrap_or(raw);
        let Some(inner) = raw
            .strip_prefix("radial-gradient(")
            .and_then(|s| s.strip_suffix(')'))
        else {
            let direction = match angle.rem_euclid(360.0) {
                0.0 => (0.0, -1.0),
                90.0 => (1.0, 0.0),
                180.0 => (0.0, 1.0),
                270.0 => (-1.0, 0.0),
                _ => (angle.to_radians().sin(), -angle.to_radians().cos()),
            };
            return Some(Self::Linear {
                direction,
                length: (size.0 * direction.0).abs() + (size.1 * direction.1).abs(),
            });
        };
        let parts = crate::style::css::split_args(inner);
        let head = parts.first()?.trim();
        let head = if crate::style::values::color_space::interpolation_color(
            super::split_top(head).first()?,
            vcx,
        )
        .is_some()
        {
            ""
        } else {
            head
        };
        let head = head.split(" in ").next().unwrap_or(head);
        let (shape, position) = head.split_once("at ").unwrap_or((head, "center"));
        let pos = parse_pos_words(position, vcx);
        let center = (len_px(pos.x, size.0, vcx)?, len_px(pos.y, size.1, vcx)?);
        let words = super::split_top(shape);
        let explicit: Vec<_> = words
            .iter()
            .filter_map(|w| Len::parse_mixed(w, vcx).filter(|l| !l.needs_unit_context(vcx)))
            .collect();
        let circle =
            words.contains(&"circle") || (explicit.len() == 1 && !words.contains(&"ellipse"));
        let radii = if let Some(first) = explicit.first() {
            let x = len_px(Some(*first), size.0, vcx)?.max(0.0);
            let y = explicit
                .get(1)
                .map_or(Some(x), |v| len_px(Some(*v), size.1, vcx))?
                .max(0.0);
            (x, y)
        } else {
            let closest = words.contains(&"closest-side") || words.contains(&"closest-corner");
            let choose = |a: f32, b: f32| if closest { a.min(b) } else { a.max(b) };
            let (x, y) = (
                choose(center.0.abs(), (size.0 - center.0).abs()),
                choose(center.1.abs(), (size.1 - center.1).abs()),
            );
            let side = if circle {
                let r = choose(x, y);
                (r, r)
            } else {
                (x, y)
            };
            if words.contains(&"closest-side") || words.contains(&"farthest-side") {
                side
            } else {
                let mut corner = (0.0, 0.0);
                let mut distance = f32::NAN;
                for (cx, cy) in [(0.0, 0.0), (size.0, 0.0), size, (0.0, size.1)] {
                    let offset = (cx - center.0, cy - center.1);
                    let d = offset.0.hypot(offset.1);
                    if distance.is_nan() || choose(d, distance) == d {
                        corner = offset;
                        distance = d;
                    }
                }
                if circle {
                    (distance, distance)
                } else if side.0 > 0.0 && side.1 > 0.0 {
                    let factor = (corner.0 / side.0).hypot(corner.1 / side.1);
                    (side.0 * factor, side.1 * factor)
                } else {
                    side
                }
            }
        };
        Some(Self::Radial { center, radii })
    }

    pub(super) fn length(&self) -> f32 {
        match self {
            Self::Linear { length, .. } => *length,
            Self::Radial { radii, .. } => radii.0,
        }
    }

    pub(super) fn at(&self, point: (f32, f32), size: (f32, f32)) -> f32 {
        match self {
            Self::Linear {
                direction: (dx, dy),
                length,
            } => {
                ((point.0 - size.0 * 0.5) * dx + (point.1 - size.1 * 0.5) * dy)
                    / length.max(f32::EPSILON)
                    + 0.5
            }
            Self::Radial { center, radii } => {
                if radii.0 <= 0.0 || radii.1 <= 0.0 {
                    return f32::INFINITY;
                }
                ((point.0 - center.0) / radii.0).hypot((point.1 - center.1) / radii.1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_extent_uses_center_and_ellipse_ratio() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let size = (300.0, 200.0);
        let g = Geometry::new(
            "radial-gradient(closest-side at 25% 50%, red, blue)",
            size,
            0.0,
            vcx,
        )
        .unwrap();
        assert_eq!(g.length(), 75.0);
        assert_eq!(g.at((150.0, 100.0), size), 1.0);
        assert_eq!(g.at((75.0, 0.0), size), 1.0);
        let g = Geometry::new("radial-gradient(red, blue)", size, 0.0, vcx).unwrap();
        assert!((g.at((300.0, 200.0), size) - 1.0).abs() < 0.00001);
    }
}
