//! Строчная ось абсолютного вертикального текста: свой заданный размер либо внутренняя
//! длина в пределах доступного. Статическая позиция в grid/flex и растяжка между двумя
//! вставками остаются своими договорами и сюда не берутся.
use crate::style::computed::orthogonal::InlineConstraint;
use crate::style::computed::{BoxStyle, Position};
use crate::style::values::value::Len;

pub(super) fn constraint(style: &BoxStyle, available: f32) -> Option<InlineConstraint> {
    let positioned = matches!(
        style.reset_positioned.position,
        Some(Position::Absolute | Position::Fixed)
    ) || style.reset_positioned.abs_static;
    let definite = |value| !matches!(value, None | Some(Len::Auto));
    if !positioned
        || !style.wm().vertical()
        || style.reset_context.ortho_col
        || style.context.parent_flex_grid
    {
        return None;
    }
    let own = crate::layout::writing_mode::orthogonal_inline::axis(style, true);
    // The inline atomic path can retain a containing-block wrapping fallback.
    // A definite physical height replaces it before RTL text alignment.
    if own.size.is_none()
        && (definite(style.reset_box_layout.height)
            || definite(style.reset_positioned.inset.top)
                && definite(style.reset_positioned.inset.bottom))
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
    use crate::style::computed::Computed;
    #[test]
    fn free_absolute_inline_size_uses_intrinsics_instead_of_zero() {
        let style = {
            let mut with_inherited = Computed {
                reset_positioned: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetPositioned {
                        position: Some(Position::Absolute),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.writing.writing_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
            );
            BoxStyle::new(with_inherited)
        };
        let c = constraint(&style, 600.0).unwrap();
        assert_eq!(c.used(120.0, 120.0), 120.0);
        assert_eq!(c.used(700.0, 700.0), 700.0);
        assert_eq!(c.used(30.0, 1000.0), 600.0);
        let mut static_style = style;
        static_style.reset_positioned.position = None;
        static_style.reset_positioned.abs_static = true;
        assert!(constraint(&static_style, 600.0).is_some());
    }
    #[test]
    fn two_auto_size_insets_and_parent_layout_keep_their_contracts() {
        let mut style = {
            let mut with_inherited = Computed {
                reset_positioned: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetPositioned {
                        position: Some(Position::Absolute),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.writing.writing_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
            );
            BoxStyle::new(with_inherited)
        };
        style.reset_box_layout.height = Some(Len::Px(200.0));
        assert_eq!(constraint(&style, 600.0).unwrap().fixed, Some(200.0));
        style.reset_box_layout.height = None;
        style.reset_positioned.inset.top = Some(Len::Px(10.0));
        style.reset_positioned.inset.bottom = Some(Len::Px(20.0));
        assert!(constraint(&style, 600.0).is_none());
        style.reset_positioned.inset.bottom = None;
        style.context.parent_flex_grid = true;
        assert!(constraint(&style, 600.0).is_none());
        style.context.parent_flex_grid = false;
        style.reset_positioned.position = None;
        style.reset_context.hug_inline = true;
        assert!(constraint(&style, 600.0).is_none());
    }

    #[test]
    fn definite_absolute_inline_size_replaces_fallback_before_alignment() {
        let mut style = {
            let mut with_inherited = Computed {
                reset_positioned: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetPositioned {
                        position: Some(Position::Absolute),
                        ..Default::default()
                    },
                ),
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        height: Some(Len::Px(80.0)),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.writing.writing_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
            );
            with_inherited.inh.writing.rtl = Some(true);
            BoxStyle::new(with_inherited)
        };
        style.reset_positioned.inset.top = Some(Len::Px(160.0));
        style.reset_positioned.inset.bottom = Some(Len::Px(160.0));
        style.reset_box_layout.min_height = Some(Len::Px(90.0));
        style.reset_box_layout.max_height = Some(Len::Px(100.0));
        let c = constraint(&style, 320.0).unwrap();
        assert_eq!(c.fixed, Some(80.0));
        assert_eq!(c.used(80.0, 80.0), 90.0);
        style.reset_border.border_box = Some(true);
        style.reset_box_layout.padding.top = Some(Len::Px(10.0));
        style.reset_box_layout.padding.bottom = Some(Len::Px(10.0));
        let c = constraint(&style, 320.0).unwrap();
        assert_eq!(c.fixed, Some(60.0));
        assert_eq!(c.used(80.0, 80.0), 70.0);
    }
}
