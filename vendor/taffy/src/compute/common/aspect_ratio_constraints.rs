//! Ratio-transferred bounds respect definite preferred and explicit opposite bounds.
use crate::{MaybeMath, Size};

/// CSS Sizing 4 §4.4: transferred bounds never override definite destination sizes.
pub(crate) fn transfer(
    preferred: Size<Option<f32>>,
    minimum: Size<Option<f32>>,
    maximum: Size<Option<f32>>,
    ratio: Option<f32>,
) -> (Size<Option<f32>>, Size<Option<f32>>) {
    let inferred_min = minimum.maybe_apply_aspect_ratio(ratio);
    let cap_min = |original: Option<f32>, inferred: Option<f32>, preferred, maximum| {
        original.or_else(|| inferred.map(|value| value.maybe_min(preferred).maybe_min(maximum)))
    };
    let minimum = Size {
        width: cap_min(
            minimum.width,
            inferred_min.width,
            preferred.width,
            maximum.width,
        ),
        height: cap_min(
            minimum.height,
            inferred_min.height,
            preferred.height,
            maximum.height,
        ),
    };
    let inferred_max = maximum.maybe_apply_aspect_ratio(ratio);
    let floor_max = |original: Option<f32>, inferred: Option<f32>, preferred, minimum| {
        original.or_else(|| inferred.map(|value| value.maybe_max(preferred).maybe_max(minimum)))
    };
    let maximum = Size {
        width: floor_max(
            maximum.width,
            inferred_max.width,
            preferred.width,
            minimum.width,
        ),
        height: floor_max(
            maximum.height,
            inferred_max.height,
            preferred.height,
            minimum.height,
        ),
    };
    (minimum, maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transferred_maximum_cannot_reduce_definite_preferred_size() {
        let preferred = Size {
            width: Some(200.0),
            height: None,
        };
        let max = Size {
            width: None,
            height: Some(100.0),
        };
        assert_eq!(
            transfer(preferred, Size::NONE, max, Some(1.0)).1.width,
            Some(200.0)
        );
    }

    #[test]
    fn transferred_minimum_is_capped_by_definite_preferred_size() {
        let preferred = Size {
            width: Some(50.0),
            height: None,
        };
        let min = Size {
            width: None,
            height: Some(100.0),
        };
        assert_eq!(
            transfer(preferred, min, Size::NONE, Some(1.0)).0.width,
            Some(50.0)
        );
    }

    #[test]
    fn explicit_destination_bounds_remain_independent() {
        let min = Size {
            width: Some(20.0),
            height: Some(100.0),
        };
        let max = Size {
            width: Some(70.0),
            height: Some(150.0),
        };
        assert_eq!(transfer(Size::NONE, min, max, Some(2.0)), (min, max));
    }

    #[test]
    fn transferred_minimum_is_capped_by_destination_maximum() {
        let min = Size {
            width: None,
            height: Some(100.0),
        };
        let max = Size {
            width: Some(70.0),
            height: None,
        };
        assert_eq!(
            transfer(Size::NONE, min, max, Some(2.0)).0.width,
            Some(70.0)
        );
    }

    #[test]
    fn transferred_maximum_respects_destination_minimum() {
        let min = Size {
            width: Some(200.0),
            height: None,
        };
        let max = Size {
            width: None,
            height: Some(100.0),
        };
        assert_eq!(
            transfer(Size::NONE, min, max, Some(1.0)).1.width,
            Some(200.0)
        );
    }

    #[test]
    fn unconstrained_destination_still_receives_ratio_bounds() {
        let min = Size {
            width: None,
            height: Some(40.0),
        };
        let max = Size {
            width: None,
            height: Some(100.0),
        };
        let bounds = transfer(Size::NONE, min, max, Some(2.0));
        assert_eq!(bounds.0.width, Some(80.0));
        assert_eq!(bounds.1.width, Some(200.0));
        assert_eq!(transfer(Size::NONE, min, max, None), (min, max));
    }
}
