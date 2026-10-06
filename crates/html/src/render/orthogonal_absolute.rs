//! Absolute vertical text uses its own definite inline size or intrinsic length.
//! Keep grid/flex static-position and two-inset stretch contracts separate.
use crate::computed::orthogonal::InlineConstraint;
use crate::computed::{Computed, Position};
use crate::value::Len;

pub(super) fn constraint(style: &Computed, available: f32) -> Option<InlineConstraint> {
    let positioned =
        matches!(style.position, Some(Position::Absolute | Position::Fixed)) || style.abs_static;
    let definite = |value| !matches!(value, None | Some(Len::Auto));
    if !positioned || style.vertical != Some(true) || style.ortho_col || style.parent_flex_grid {
        return None;
    }
    let own = super::orthogonal_inline::axis(style, true);
    // The inline atomic path can retain a containing-block wrapping fallback.
    // A definite physical height replaces it before RTL text alignment.
    if own.size.is_none()
        && (definite(style.height) || definite(style.inset.top) && definite(style.inset.bottom))
    {
        return None;
    }
    Some(InlineConstraint {
        available: available.max(0.0),
        fixed: own.size,
        min: own.min,
        max: own.max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn free_absolute_inline_size_uses_intrinsics_instead_of_zero() {
        let style = Computed {
            position: Some(Position::Absolute),
            vertical: Some(true),
            ..Computed::default()
        };
        let c = constraint(&style, 600.0).unwrap();
        assert_eq!(c.used(120.0, 120.0), 120.0);
        assert_eq!(c.used(700.0, 700.0), 700.0);
        assert_eq!(c.used(30.0, 1000.0), 600.0);
        let static_style = Computed {
            position: None,
            abs_static: true,
            ..style
        };
        assert!(constraint(&static_style, 600.0).is_some());
    }
    #[test]
    fn two_auto_size_insets_and_parent_layout_keep_their_contracts() {
        let mut style = Computed {
            position: Some(Position::Absolute),
            vertical: Some(true),
            ..Computed::default()
        };
        style.height = Some(Len::Px(200.0));
        assert_eq!(constraint(&style, 600.0).unwrap().fixed, Some(200.0));
        style.height = None;
        style.inset.top = Some(Len::Px(10.0));
        style.inset.bottom = Some(Len::Px(20.0));
        assert!(constraint(&style, 600.0).is_none());
        style.inset.bottom = None;
        style.parent_flex_grid = true;
        assert!(constraint(&style, 600.0).is_none());
        style.parent_flex_grid = false;
        style.position = None;
        style.hug_inline = true;
        assert!(constraint(&style, 600.0).is_none());
    }

    #[test]
    fn definite_absolute_inline_size_replaces_fallback_before_alignment() {
        let mut style = Computed {
            position: Some(Position::Absolute),
            vertical: Some(true),
            rtl: Some(true),
            height: Some(Len::Px(80.0)),
            ..Computed::default()
        };
        style.inset.top = Some(Len::Px(160.0));
        style.inset.bottom = Some(Len::Px(160.0));
        style.min_height = Some(Len::Px(90.0));
        style.max_height = Some(Len::Px(100.0));
        let c = constraint(&style, 320.0).unwrap();
        assert_eq!(c.fixed, Some(80.0));
        assert_eq!(c.used(80.0, 80.0), 90.0);
        style.border_box = Some(true);
        style.padding.top = Some(Len::Px(10.0));
        style.padding.bottom = Some(Len::Px(10.0));
        let c = constraint(&style, 320.0).unwrap();
        assert_eq!(c.fixed, Some(60.0));
        assert_eq!(c.used(80.0, 80.0), 70.0);
    }
}
