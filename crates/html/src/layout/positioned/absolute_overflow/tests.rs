//! Тесты плана безопасного выравнивания абсолюта: выбор статической оси по режиму письма
//! и направлению flex-родителя и отказ от плана при заданных вставках или локальном CB.
use super::*;
use crate::style::computed::{BoxStyle, Computed};
#[test]
fn static_cross_axis_tracks_parent_writing_mode_and_direction() {
    let own = Computed {
        reset_positioned: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetPositioned {
                position: Some(Position::Absolute),
                ..Default::default()
            },
        ),
        reset_flex: crate::style::computed::StyleGroup::new(crate::style::computed::ResetFlex {
            align_self: Some(Align::Center),
            align_self_safe: true,
            ..Default::default()
        }),
        ..Computed::default()
    };
    let mut parent = BoxStyle::from(Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                display: Some(Display::Flex),
                ..Default::default()
            },
        ),
        ..Computed::default()
    });
    parent.self_node = 1;
    assert!(
        !Plan::new(&BoxStyle::from(own.clone()), &parent, &Default::default())
            .unwrap()
            .horizontal
    );
    parent.reset_flex.flex_dir = Some(FlexDir::Col);
    assert!(
        Plan::new(&BoxStyle::from(own.clone()), &parent, &Default::default())
            .unwrap()
            .horizontal
    );
    parent.inh.writing.writing_mode =
        Some(crate::style::computed::writing_mode::WritingModeKw::new(
            true,
            parent.wm().block_rl(),
            parent.wm().sideways(),
        ));
    assert!(
        !Plan::new(&BoxStyle::from(own.clone()), &parent, &Default::default())
            .unwrap()
            .horizontal
    );
    parent.reset_flex.flex_dir = Some(FlexDir::Row);
    parent.inh.writing.writing_mode = Some(
        crate::style::computed::writing_mode::WritingModeKw::new(true, true, false),
    );
    let plan = Plan::new(&BoxStyle::from(own.clone()), &parent, &Default::default()).unwrap();
    assert!(plan.horizontal && plan.from_end);
    let mut inset = own.clone();
    inset.reset_positioned.inset.left = Some(Len::Px(0.0));
    assert!(Plan::new(&BoxStyle::from(inset), &parent, &Default::default()).is_none());
    let mut local_cb = BoxStyle::from(own);
    local_cb.cb_node = parent.self_node;
    assert!(Plan::new(&local_cb, &parent, &Default::default()).is_none());
}
