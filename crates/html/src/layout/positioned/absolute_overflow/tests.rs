//! Verify abspos static-axis projection and authored inset boundaries.
use super::*;
#[test]
fn static_cross_axis_tracks_parent_writing_mode_and_direction() {
    let own = Computed {
        position: Some(Position::Absolute),
        align_self: Some(Align::Center),
        align_self_safe: true,
        ..Computed::default()
    };
    let mut parent = Computed {
        display: Some(Display::Flex),
        self_node: 1,
        ..Computed::default()
    };
    assert!(!Plan::new(&own, &parent).unwrap().horizontal);
    parent.flex_dir = Some(FlexDir::Col);
    assert!(Plan::new(&own, &parent).unwrap().horizontal);
    parent.vertical = Some(true);
    assert!(!Plan::new(&own, &parent).unwrap().horizontal);
    parent.flex_dir = Some(FlexDir::Row);
    parent.vertical_rl = Some(true);
    let plan = Plan::new(&own, &parent).unwrap();
    assert!(plan.horizontal && plan.from_end);
    let mut inset = own.clone();
    inset.inset.left = Some(Len::Px(0.0));
    assert!(Plan::new(&inset, &parent).is_none());
    let mut local_cb = own;
    local_cb.cb_node = parent.self_node;
    assert!(Plan::new(&local_cb, &parent).is_none());
}
