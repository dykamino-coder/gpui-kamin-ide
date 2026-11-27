//! Тесты ограничений inline-размера в ортогональных потоках.

use super::*;
#[test]
fn parallel_table_column_uses_its_track_measure() {
    let mut style = Computed {
        reset_context: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetContext {
                ortho_col: true,
                orthogonal_inline: Some(InlineConstraint {
                    available: 600.0,
                    fixed: None,
                    min: None,
                    max: None,
                }),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    assert_eq!(flow_constraint(&style), None);
    style.reset_context.ortho_col = false;
    assert_eq!(
        flow_constraint(&style),
        style.reset_context.orthogonal_inline
    );
}
#[test]
fn percentage_uses_fallback_but_fixed_parent_keeps_its_own_basis() {
    let child = {
        let mut with_inherited = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    height: Some(Len::Pct(0.5)),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let mut child = BoxStyle::from(child);
    resolve(&mut child, &BoxStyle::default(), (800.0, 600.0), false);
    assert_eq!(child.reset_box_layout.height, Some(Len::Px(300.0)));
    let parent = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                height: Some(Len::Px(720.0)),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    child.reset_box_layout.height = Some(Len::Pct(0.5));
    resolve(&mut child, &BoxStyle::from(parent), (800.0, 600.0), false);
    assert_eq!(child.reset_box_layout.height, Some(Len::Px(360.0)));
}

#[test]
fn nearest_indefinite_scroller_replaces_an_outer_fixed_scroller() {
    let mut outer = BoxStyle::default();
    outer.tree.orthogonal_scrollport = Some([
        AxisSizes::default(),
        AxisSizes {
            size: Some(100.0),
            ..AxisSizes::default()
        },
    ]);
    let inner = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                overflow_y: Some(Overflow::Hidden),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    let mut inner = BoxStyle::from(inner);
    resolve(&mut inner, &outer, (800.0, 600.0), false);
    assert_eq!(inner.tree.orthogonal_scrollport.unwrap()[1].size, None);
    let child = {
        let mut with_inherited = Computed {
            ..Computed::default()
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let mut child = BoxStyle::from(child);
    resolve(&mut child, &inner, (800.0, 600.0), false);
    assert_eq!(
        child.reset_context.orthogonal_inline.unwrap().available,
        600.0
    );
}

#[test]
fn inherited_writing_mode_does_not_establish_another_orthogonal_boundary() {
    let c = InlineConstraint {
        available: 100.0,
        fixed: None,
        min: None,
        max: None,
    };
    let parent = {
        let mut with_inherited = Computed {
            reset_context: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetContext {
                    orthogonal_inline: Some(c),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let child = {
        let mut with_inherited = Computed {
            ..Computed::default()
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    let mut child = BoxStyle::from(child);
    resolve(
        &mut child,
        &BoxStyle::from(parent.clone()),
        (800.0, 600.0),
        false,
    );
    assert_eq!(child.reset_context.orthogonal_inline, Some(c));
    child.reset_box_layout.height = Some(Len::Px(40.0));
    resolve(
        &mut child,
        &BoxStyle::from(parent.clone()),
        (800.0, 600.0),
        false,
    );
    assert_eq!(
        child.reset_context.orthogonal_inline.unwrap().fixed,
        Some(40.0)
    );
    child.reset_box_layout.display = Some(Display::Flex);
    resolve(&mut child, &BoxStyle::from(parent), (800.0, 600.0), false);
    assert_eq!(child.reset_context.orthogonal_inline, None);
}

#[test]
fn percentage_margins_use_cb_width_and_clip_does_not_replace_scrollport() {
    let mut parent = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                width: Some(Len::Px(500.0)),
                height: Some(Len::Px(200.0)),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    let mut child = {
        let mut with_inherited = Computed {
            ..Computed::default()
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, false, false),
        );
        with_inherited
    };
    child.reset_box_layout.margin.top = Some(Len::Pct(0.1));
    child.reset_box_layout.margin.bottom = Some(Len::Pct(0.1));
    let mut child = BoxStyle::from(child);
    resolve(
        &mut child,
        &BoxStyle::from(parent.clone()),
        (800.0, 600.0),
        false,
    );
    assert_eq!(
        child.reset_context.orthogonal_inline.unwrap().available,
        100.0
    );
    parent.reset_border.border_box = Some(true);
    parent.reset_box_layout.padding.left = Some(Len::Px(10.0));
    parent.reset_box_layout.padding.right = Some(Len::Px(10.0));
    resolve(
        &mut child,
        &BoxStyle::from(parent.clone()),
        (800.0, 600.0),
        false,
    );
    assert_eq!(
        child.reset_context.orthogonal_inline.unwrap().available,
        104.0
    );
    let clipped = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                overflow_y: Some(Overflow::Clip),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    let mut outer = BoxStyle::default();
    outer.tree.orthogonal_scrollport = Some([
        AxisSizes::default(),
        AxisSizes {
            size: Some(200.0),
            ..AxisSizes::default()
        },
    ]);
    let mut clipped = BoxStyle::from(clipped);
    resolve(&mut clipped, &outer, (800.0, 600.0), false);
    assert_eq!(
        clipped.tree.orthogonal_scrollport,
        outer.tree.orthogonal_scrollport
    );
}
