//! Тесты плана безопасного выравнивания абсолюта: выбор статической оси по режиму письма
//! и направлению flex-родителя и отказ от плана при заданных вставках или локальном CB.
use super::*;
use crate::style::computed::Computed;
#[test]
fn static_cross_axis_tracks_parent_writing_mode_and_direction() {
    let own = Computed {
        position: Some(Position::Absolute),
        align_self: Some(Align::Center),
        align_self_safe: true,
        ..Computed::default()
    };
    let mut parent = crate::style::computed::BoxStyle::from(Computed {
        display: Some(Display::Flex),
        ..Computed::default()
    });
    parent.self_node = 1;
    assert!(
        !Plan::new(
            &crate::style::computed::BoxStyle::from(own.clone()),
            &parent,
            &Default::default()
        )
        .unwrap()
        .horizontal
    );
    parent.flex_dir = Some(FlexDir::Col);
    assert!(
        Plan::new(
            &crate::style::computed::BoxStyle::from(own.clone()),
            &parent,
            &Default::default()
        )
        .unwrap()
        .horizontal
    );
    parent.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
        true,
        parent.wm().block_rl(),
        parent.wm().sideways(),
    ));
    assert!(
        !Plan::new(
            &crate::style::computed::BoxStyle::from(own.clone()),
            &parent,
            &Default::default()
        )
        .unwrap()
        .horizontal
    );
    parent.flex_dir = Some(FlexDir::Row);
    parent.writing_mode = Some(crate::style::computed::writing_mode::WritingModeKw::new(
        true, true, false,
    ));
    let plan = Plan::new(
        &crate::style::computed::BoxStyle::from(own.clone()),
        &parent,
        &Default::default(),
    )
    .unwrap();
    assert!(plan.horizontal && plan.from_end);
    let mut inset = own.clone();
    inset.inset.left = Some(Len::Px(0.0));
    assert!(
        Plan::new(
            &crate::style::computed::BoxStyle::from(inset.clone()),
            &parent,
            &Default::default()
        )
        .is_none()
    );
    let mut local_cb = crate::style::computed::BoxStyle::from(own);
    local_cb.cb_node = parent.self_node;
    assert!(Plan::new(&local_cb, &parent, &Default::default()).is_none());
}
