//! Проверки поведения доменного модуля.
use super::*;

fn constraint(style: &Computed, fallback: f32) -> Option<InlineConstraint> {
    super::constraint(
        &crate::style::computed::BoxStyle::new(style.clone()),
        fallback,
    )
}

#[test]
fn float_claim_follows_own_writing_mode_and_preserves_definite_inline_sizes() {
    let parent = {
        let mut with_inherited = Computed {
            ..Computed::default()
        };
        with_inherited.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let mut child = Computed::default();
    claim_float_inline_size(&mut child, &parent, &[]);
    assert!(child.hug_inline && child.hug_claim);
    assert_eq!(child.height, Some(Len::FitContent));
    let mut horizontal = {
        let mut with_inherited = Computed {
            ..Computed::default()
        };
        with_inherited.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(false, false, false),
        );
        with_inherited
    };
    claim_float_inline_size(&mut horizontal, &parent, &[]);
    assert!(!horizontal.hug_inline);
    let mut fixed = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                height: Some(Len::Px(70.0)),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    claim_float_inline_size(&mut fixed, &parent, &[]);
    assert!(!fixed.hug_inline);
}

#[test]
fn auto_shrink_to_fit_claim_uses_real_intrinsics_and_own_limits() {
    let style = {
        let mut with_inherited = Computed {
            reset_context: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetContext {
                    hug_inline: true,
                    ..Default::default()
                },
            ),
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    min_height: Some(Len::Px(80.0)),
                    max_height: Some(Len::Px(250.0)),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let value = constraint(&style, 600.0).unwrap();
    assert_eq!(value.used_keyword(None, 40.0, 180.0), 180.0);
    assert_eq!(value.used_keyword(None, 40.0, 900.0), 250.0);
    assert_eq!(value.used_keyword(None, 40.0, 60.0), 80.0);
}

#[test]
fn authored_inline_size_remains_definite_without_an_orthogonal_boundary() {
    let style = {
        let mut with_inherited = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    display: Some(crate::style::computed::Display::InlineBlock),
                    height: Some(Len::Px(160.0)),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let value = constraint(&style, 600.0).unwrap();
    assert_eq!(value.fixed, Some(160.0));
    assert_eq!(
        value.used_keyword(Some(InlineKeyword::MinContent), 40.0, 700.0),
        160.0
    );
}

#[test]
fn parallel_table_track_keeps_min_content_and_own_limits() {
    let mut style = {
        let mut with_inherited = Computed {
            reset_context: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetContext {
                    ortho_col: true,
                    ortho_limit: Some(200.0),
                    ..Default::default()
                },
            ),
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    min_height: Some(Len::Px(40.0)),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let value = constraint(&style, 200.0).unwrap();
    assert_eq!(value.used_keyword(keyword(&style), 30.0, 900.0), 40.0);
    style.height = Some(Len::Px(80.0));
    assert_eq!(
        constraint(&style, 200.0)
            .unwrap()
            .used_keyword(keyword(&style), 30.0, 900.0),
        80.0
    );
}

#[test]
fn ordinary_intrinsic_height_does_not_become_a_table_minimum() {
    let style = {
        let mut with_inherited = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    height: Some(Len::MaxContent),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let value = constraint(&style, 200.0).unwrap();
    assert_eq!(value.used_keyword(keyword(&style), 30.0, 900.0), 900.0);
    let intrinsic_table = {
        let style_base = style;
        Computed {
            reset_context: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetContext {
                    ortho_col: true,
                    ortho_limit: Some(200.0),
                    ..(*style_base.reset_context).clone()
                },
            ),
            ..style_base
        }
    };
    assert_eq!(
        constraint(&intrinsic_table, 200.0).unwrap().used_keyword(
            keyword(&intrinsic_table),
            30.0,
            900.0
        ),
        900.0
    );
    let table = Computed {
        reset_context: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetContext {
                ortho_col: true,
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    assert!(constraint(&table, 200.0).is_none());
}
