//! Infer only automatic ratio height, then respect independent min/max bounds.
use crate::{MaybeMath, Size};

pub(super) fn size(
    clamped: Size<f32>,
    known_height: Option<f32>,
    ratio: Option<f32>,
    box_adjustment: Size<f32>,
    minimum: Size<Option<f32>>,
    maximum: Size<Option<f32>>,
) -> Size<f32> {
    let inferred = if known_height.is_none() {
        ratio.map(|ratio| {
            (clamped.width - box_adjustment.width).max(0.0) / ratio + box_adjustment.height
        })
    } else {
        None
    };
    Size {
        width: clamped.width,
        height: clamped
            .height
            .maybe_max(inferred)
            .maybe_clamp(minimum.height, maximum.height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_height_is_independent_of_ratio() {
        assert_eq!(
            size(
                Size {
                    width: 200.0,
                    height: 100.0
                },
                Some(100.0),
                Some(1.0),
                Size::ZERO,
                Size::NONE,
                Size::NONE
            ),
            Size {
                width: 200.0,
                height: 100.0
            }
        );
    }

    #[test]
    fn inferred_height_is_limited_by_maximum() {
        let maximum = Size {
            width: None,
            height: Some(100.0),
        };
        assert_eq!(
            size(
                Size {
                    width: 200.0,
                    height: 0.0
                },
                None,
                Some(1.0),
                Size::ZERO,
                Size::NONE,
                maximum
            )
            .height,
            100.0
        );
    }

    #[test]
    fn content_box_ratio_uses_content_dimensions() {
        let adjustment = Size {
            width: 10.0,
            height: 20.0,
        };
        assert_eq!(
            size(
                Size {
                    width: 210.0,
                    height: 20.0
                },
                None,
                Some(1.0),
                adjustment,
                Size::NONE,
                Size::NONE
            )
            .height,
            220.0
        );
    }

    #[test]
    fn minimum_wins_over_conflicting_maximum() {
        let minimum = Size {
            width: None,
            height: Some(200.0),
        };
        let maximum = Size {
            width: None,
            height: Some(100.0),
        };
        assert_eq!(
            size(
                Size {
                    width: 50.0,
                    height: 0.0
                },
                None,
                Some(1.0),
                Size::ZERO,
                minimum,
                maximum
            )
            .height,
            200.0
        );
    }
}
