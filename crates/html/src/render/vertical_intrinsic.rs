//! Physical height is the inline axis of vertical text, including intrinsic sizes.
//! CSS Sizing 3 §5.1: max-content is not capped by available inline space.
use crate::computed::Computed;
use crate::computed::orthogonal::{InlineConstraint, InlineKeyword};
use crate::value::Len;

pub(super) fn constraint(
    style: &Computed,
    available: f32,
) -> Option<(InlineKeyword, InlineConstraint)> {
    if style.vertical != Some(true) || style.contains_height() {
        return None;
    }
    let keyword = match style.height {
        Some(Len::MinContent) => InlineKeyword::MinContent,
        Some(Len::MaxContent) => InlineKeyword::MaxContent,
        Some(Len::FitContent) => InlineKeyword::FitContent,
        _ => return None,
    };
    let axis = super::orthogonal_inline::axis(style, true);
    Some((
        keyword,
        InlineConstraint {
            available: style
                .orthogonal_inline
                .map_or(available, |c| c.available)
                .max(0.0),
            fixed: None,
            min: axis.min,
            max: axis.max,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vertical_intrinsic_modes_use_text_contributions_without_viewport_clamping() {
        let mut style = Computed {
            vertical: Some(true),
            ..Computed::default()
        };
        for (height, expected) in [
            (Len::MinContent, 30.0),
            (Len::MaxContent, 900.0),
            (Len::FitContent, 600.0),
        ] {
            style.height = Some(height);
            let (keyword, c) = constraint(&style, 600.0).unwrap();
            assert_eq!(c.used_keyword(Some(keyword), 30.0, 900.0), expected);
        }
        style.min_height = Some(Len::Px(700.0));
        style.max_height = Some(Len::Px(500.0));
        let (keyword, c) = constraint(&style, 600.0).unwrap();
        assert_eq!(c.used_keyword(Some(keyword), 30.0, 900.0), 700.0);
    }

    #[test]
    fn auto_fixed_horizontal_and_containment_do_not_claim_text_intrinsics() {
        let mut style = Computed {
            vertical: Some(true),
            ..Computed::default()
        };
        assert!(constraint(&style, 600.0).is_none());
        style.height = Some(Len::Px(100.0));
        assert!(constraint(&style, 600.0).is_none());
        style.height = Some(Len::MinContent);
        style.vertical = None;
        assert!(constraint(&style, 600.0).is_none());
        style.vertical = Some(true);
        style.contain_size = Some(true);
        assert!(constraint(&style, 600.0).is_none());
    }
}
