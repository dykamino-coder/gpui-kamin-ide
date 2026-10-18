//! Computed gradient directions and radial ending shapes; image size stays unresolved.

use super::{BgPos, Len, parse_pos_words};

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum GradientShape {
    #[default]
    Linear,
    Corner(f32, f32),
    Radial {
        position: BgPos,
        circle: bool,
        size: RadialSize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RadialSize {
    Explicit(Len, Option<Len>),
    Extent(RadialExtent),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RadialExtent {
    ClosestSide,
    FarthestSide,
    ClosestCorner,
    FarthestCorner,
}

impl GradientShape {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let raw = raw.strip_prefix("repeating-").unwrap_or(raw);
        let Some(inner) = raw
            .strip_prefix("radial-gradient(")
            .and_then(|s| s.strip_suffix(')'))
        else {
            return Some(
                crate::style::computed::gradient::corner_direction(raw)
                    .map_or(Self::Linear, |(x, y)| Self::Corner(x, y)),
            );
        };
        let parts = crate::style::css::split_args(inner);
        let head = parts.first()?.trim();
        let head = if crate::style::css::value_tokens::split_top(head)
            .first()
            .and_then(|s| crate::style::values::color_space::ColorExpression::parse(s))
            .is_some()
        {
            ""
        } else {
            head
        };
        let head = head.split(" in ").next().unwrap_or(head);
        let (shape, position) = head.split_once("at ").unwrap_or((head, "center"));
        let words = crate::style::css::value_tokens::split_top(shape);
        let explicit: Vec<_> = words
            .iter()
            .filter_map(|w| Len::parse_mixed(w).filter(|l| !l.needs_unit_context()))
            .collect();
        let circle =
            words.contains(&"circle") || (explicit.len() == 1 && !words.contains(&"ellipse"));
        let size = if let Some(first) = explicit.first() {
            RadialSize::Explicit(*first, explicit.get(1).copied())
        } else {
            RadialSize::Extent(if words.contains(&"closest-side") {
                RadialExtent::ClosestSide
            } else if words.contains(&"farthest-side") {
                RadialExtent::FarthestSide
            } else if words.contains(&"closest-corner") {
                RadialExtent::ClosestCorner
            } else {
                RadialExtent::FarthestCorner
            })
        };
        Some(Self::Radial {
            position: parse_pos_words(position),
            circle,
            size,
        })
    }

    pub(crate) fn corner_angle(&self, (width, height): (f32, f32)) -> Option<f32> {
        match self {
            Self::Corner(x, y) => Some(
                (x * height)
                    .atan2(-y * width)
                    .to_degrees()
                    .rem_euclid(360.0),
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radial_shape_retains_radii_position_and_extent() {
        let shape = GradientShape::parse(
            "radial-gradient(ellipse 20px 40% at 25% 75% in oklch longer hue, red, blue)",
        )
        .unwrap();
        assert_eq!(
            shape,
            GradientShape::Radial {
                position: BgPos {
                    x: Some(Len::Pct(0.25)),
                    y: Some(Len::Pct(0.75))
                },
                circle: false,
                size: RadialSize::Explicit(Len::Px(20.0), Some(Len::Pct(0.4))),
            }
        );
        for (name, extent) in [
            ("closest-side", RadialExtent::ClosestSide),
            ("farthest-side", RadialExtent::FarthestSide),
            ("closest-corner", RadialExtent::ClosestCorner),
            ("farthest-corner", RadialExtent::FarthestCorner),
        ] {
            let raw = format!("repeating-radial-gradient(circle {name} at left top, red, blue)");
            assert_eq!(
                GradientShape::parse(&raw),
                Some(GradientShape::Radial {
                    position: BgPos {
                        x: Some(Len::Pct(0.0)),
                        y: Some(Len::Pct(0.0))
                    },
                    circle: true,
                    size: RadialSize::Extent(extent),
                })
            );
        }
    }

    #[test]
    fn corners_keep_all_quadrants_until_image_size_is_known() {
        for (words, x, y, angle) in [
            ("top right", 1.0, -1.0, 26.56505),
            ("right bottom", 1.0, 1.0, 153.43495),
            ("bottom left", -1.0, 1.0, 206.56505),
            ("left top", -1.0, -1.0, 333.43495),
        ] {
            let raw = format!("linear-gradient(to {words} in lch longer hue, red, blue)");
            let shape = GradientShape::parse(&raw).unwrap();
            assert_eq!(shape, GradientShape::Corner(x, y));
            assert!((shape.corner_angle((200.0, 100.0)).unwrap() - angle).abs() < 0.0001);
        }
    }
}
