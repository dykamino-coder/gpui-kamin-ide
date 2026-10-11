//! Тесты ограничений inline-размера в ортогональных потоках.

use super::*;
#[test]
fn parallel_table_column_uses_its_track_measure() {
    let mut style = Computed {
        ortho_col: true,
        orthogonal_inline: Some(InlineConstraint {
            available: 600.0,
            fixed: None,
            min: None,
            max: None,
        }),
        ..Computed::default()
    };
    assert_eq!(flow_constraint(&style), None);
    style.ortho_col = false;
    assert_eq!(flow_constraint(&style), style.orthogonal_inline);
}
#[test]
fn percentage_uses_fallback_but_fixed_parent_keeps_its_own_basis() {
    let mut child = Computed {
        vertical: Some(true),
        height: Some(Len::Pct(0.5)),
        ..Computed::default()
    };
    resolve(&mut child, &Computed::default(), (800.0, 600.0), false);
    assert_eq!(child.height, Some(Len::Px(300.0)));
    let parent = Computed {
        height: Some(Len::Px(720.0)),
        ..Computed::default()
    };
    child.height = Some(Len::Pct(0.5));
    resolve(&mut child, &parent, (800.0, 600.0), false);
    assert_eq!(child.height, Some(Len::Px(360.0)));
}

#[test]
fn nearest_indefinite_scroller_replaces_an_outer_fixed_scroller() {
    let outer = Computed {
        orthogonal_scrollport: Some([
            AxisSizes::default(),
            AxisSizes {
                size: Some(100.0),
                ..AxisSizes::default()
            },
        ]),
        ..Computed::default()
    };
    let mut inner = Computed {
        overflow_y: Some(Overflow::Hidden),
        ..Computed::default()
    };
    resolve(&mut inner, &outer, (800.0, 600.0), false);
    assert_eq!(inner.orthogonal_scrollport.unwrap()[1].size, None);
    let mut child = Computed {
        vertical: Some(true),
        ..Computed::default()
    };
    resolve(&mut child, &inner, (800.0, 600.0), false);
    assert_eq!(child.orthogonal_inline.unwrap().available, 600.0);
}

#[test]
fn inherited_writing_mode_does_not_establish_another_orthogonal_boundary() {
    let c = InlineConstraint {
        available: 100.0,
        fixed: None,
        min: None,
        max: None,
    };
    let parent = Computed {
        vertical: Some(true),
        orthogonal_inline: Some(c),
        ..Computed::default()
    };
    let mut child = Computed {
        vertical: Some(true),
        ..Computed::default()
    };
    resolve(&mut child, &parent, (800.0, 600.0), false);
    assert_eq!(child.orthogonal_inline, Some(c));
    child.height = Some(Len::Px(40.0));
    resolve(&mut child, &parent, (800.0, 600.0), false);
    assert_eq!(child.orthogonal_inline.unwrap().fixed, Some(40.0));
    child.display = Some(Display::Flex);
    resolve(&mut child, &parent, (800.0, 600.0), false);
    assert_eq!(child.orthogonal_inline, None);
}

#[test]
fn percentage_margins_use_cb_width_and_clip_does_not_replace_scrollport() {
    let mut parent = Computed {
        width: Some(Len::Px(500.0)),
        height: Some(Len::Px(200.0)),
        ..Computed::default()
    };
    let mut child = Computed {
        vertical: Some(true),
        ..Computed::default()
    };
    child.margin.top = Some(Len::Pct(0.1));
    child.margin.bottom = Some(Len::Pct(0.1));
    resolve(&mut child, &parent, (800.0, 600.0), false);
    assert_eq!(child.orthogonal_inline.unwrap().available, 100.0);
    parent.border_box = Some(true);
    parent.padding.left = Some(Len::Px(10.0));
    parent.padding.right = Some(Len::Px(10.0));
    resolve(&mut child, &parent, (800.0, 600.0), false);
    assert_eq!(child.orthogonal_inline.unwrap().available, 104.0);
    let mut clipped = Computed {
        overflow_y: Some(Overflow::Clip),
        ..Computed::default()
    };
    let outer = Computed {
        orthogonal_scrollport: Some([
            AxisSizes::default(),
            AxisSizes {
                size: Some(200.0),
                ..AxisSizes::default()
            },
        ]),
        ..Computed::default()
    };
    resolve(&mut clipped, &outer, (800.0, 600.0), false);
    assert_eq!(clipped.orthogonal_scrollport, outer.orthogonal_scrollport);
}
