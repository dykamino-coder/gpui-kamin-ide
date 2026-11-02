//! Проверки поведения доменного модуля.
use super::*;
use crate::style::computed::{Align, Computed, Display};

#[test]
fn own_vertical_flex_items_keep_the_parent_central_synthesis_policy() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for (rl, sideways, flags) in [
        (false, false, 6),
        (true, false, 7),
        (false, true, 4),
        (true, true, 5),
    ] {
        let parent = {
            let mut with_inherited = Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        display: Some(Display::Flex),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.writing.writing_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(true, rl, sideways),
            );
            with_inherited
        };
        let effective = crate::style::cascade::inherit::inherit(
            &BoxStyle::from(parent.clone()),
            &BoxStyle::default(),
            vcx,
        );
        assert_eq!(baseline_x_flags(&effective), Some(flags));
        let mut wrapper = gpui::StyleRefinement::default();
        project_wrapper(&mut wrapper, &effective);
        assert_eq!(wrapper.baseline_x_flags, Some(flags));
    }
}

#[test]
fn horizontal_flex_items_inherit_vertical_parent_baseline_synthesis() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    for (rl, sideways, flags) in [
        (false, false, 2),
        (true, false, 3),
        (false, true, 0),
        (true, true, 1),
    ] {
        let parent = {
            let mut with_inherited = Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        display: Some(Display::Flex),
                        ..Default::default()
                    },
                ),
                ..Computed::default()
            };
            with_inherited.inh.writing.writing_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(true, rl, sideways),
            );
            with_inherited
        };
        let own = {
            let mut with_inherited = Computed {
                ..Computed::default()
            };
            with_inherited.inh.writing.writing_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(false, false, false),
            );
            with_inherited.inh.writing.text_orientation =
                Some(crate::style::computed::writing_mode::TextOrientation::Mixed);
            with_inherited
        };
        let effective =
            crate::style::cascade::inherit::inherit(&BoxStyle::from(parent.clone()), &own, vcx);
        assert_eq!(baseline_x_flags(&effective), Some(flags));
        let mut wrapper = gpui::StyleRefinement::default();
        project_wrapper(&mut wrapper, &effective);
        assert_eq!(wrapper.baseline_x_flags, Some(flags));
    }
}

#[test]
fn flex_items_and_wrappers_preserve_own_vertical_baseline_orientation() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let parent = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                display: Some(Display::Flex),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    for (rl, flags) in [(false, 4), (true, 5)] {
        let own = {
            let mut with_inherited = Computed {
                ..Computed::default()
            };
            with_inherited.inh.writing.writing_mode = Some(
                crate::style::computed::writing_mode::WritingModeKw::new(true, rl, false),
            );
            with_inherited
        };
        let effective =
            crate::style::cascade::inherit::inherit(&BoxStyle::from(parent.clone()), &own, vcx);
        assert_eq!(baseline_x_flags(&effective), Some(flags));
        let mut wrapper = gpui::StyleRefinement {
            align_self: Some(gpui::AlignItems::End),
            ..Default::default()
        };
        project_wrapper(&mut wrapper, &effective);
        assert_eq!(wrapper.baseline_x_flags, Some(flags));
        assert_eq!(wrapper.align_self, Some(gpui::AlignItems::End));
    }
    assert_eq!(
        baseline_x_flags(&crate::style::cascade::inherit::inherit(
            &BoxStyle::from(parent),
            &BoxStyle::default(),
            vcx
        )),
        None
    );
}

#[test]
fn lanes_wrapper_retains_inherited_item_orientation() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let parent = Computed {
        reset_box_layout: crate::style::computed::StyleGroup::new(
            crate::style::computed::ResetBoxLayout {
                display: Some(Display::GridLanes),
                ..Default::default()
            },
        ),
        ..Computed::default()
    };
    let own = {
        let mut with_inherited = Computed {
            reset_flex: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetFlex {
                    justify_self: Some(Align::Baseline),
                    justify_self_last: true,
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        with_inherited.inh.writing.writing_mode = Some(
            crate::style::computed::writing_mode::WritingModeKw::new(true, true, false),
        );
        with_inherited
    };
    let effective = crate::style::cascade::inherit::inherit(&BoxStyle::from(parent), &own, vcx);
    let mut wrapper = gpui::StyleRefinement::default();
    project_wrapper(&mut wrapper, &effective);
    assert_eq!(wrapper.baseline_x_flags, Some(5));
    assert_eq!(wrapper.justify_self, Some(gpui::AlignItems::LastBaseline));
    assert_eq!(baseline_x_flags(&effective), wrapper.baseline_x_flags);
}

#[test]
fn wrapper_self_alignment_and_safety_follow_vertical_parent_axes() {
    let mut c = BoxStyle::new(Computed {
        reset_flex: crate::style::computed::StyleGroup::new(crate::style::computed::ResetFlex {
            align_self: Some(Align::Baseline),
            align_self_last: true,
            align_self_safe: true,
            ..Default::default()
        }),
        ..Computed::default()
    });
    c.context.parent_grid = 2;
    let mut wrapper = gpui::StyleRefinement::default();
    project_wrapper(&mut wrapper, &c);
    assert_eq!(wrapper.justify_self, Some(gpui::AlignItems::LastBaseline));
    assert_eq!(wrapper.safe_justify_alignment, Some((false, true)));
}
