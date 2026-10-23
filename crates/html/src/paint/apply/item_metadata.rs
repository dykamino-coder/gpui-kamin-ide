//! Применение к GPUI: метаданные элемента сетки/флекса, которые обязаны
//! переехать на анонимную обёртку размера по содержимому (`render::content_sized`)
//! — ведь в контейнере родителя стоит она. Это флаги ориентации базовой линии
//! (`baseline_x_flags`), `align-self`/`justify-self` с признаком `safe`,
//! проекция на оси вертикального родителя и baseline содержимого сетки.
use crate::style::computed::Computed;

fn grid_mode(c: &Computed) -> u8 {
    if c.parent_grid != 0 {
        c.parent_grid
    } else if c.parent_lanes {
        match (c.cb_wm.vertical(), c.cb_wm.block_rl()) {
            (false, _) => 1,
            (true, false) => 2,
            (true, true) => 3,
        }
    } else {
        0
    }
}

pub(super) fn baseline_x_flags(c: &Computed) -> Option<u8> {
    let parent = grid_mode(c);
    if parent == 0 {
        if !c.parent_flex_grid {
            return None;
        }
        let central = c.cb_wm.vertical() && !c.cb_wm.sideways() && !c.wm().text_sideways();
        if c.wm().vertical() {
            return Some(4 | u8::from(c.wm().block_rl()) | if central { 2 } else { 0 });
        }
        return c
            .cb_wm
            .vertical()
            .then(|| u8::from(c.cb_wm.block_rl()) | if central { 2 } else { 0 });
    }
    let vertical = c.wm().vertical();
    let rl = if c.parent_grid != 0
        && matches!(
            c.position,
            Some(
                crate::style::computed::Position::Absolute
                    | crate::style::computed::Position::Fixed
            )
        ) {
        // No baseline-sharing context: fallback is the item's own self-start.
        super::grid_flow_axes::reversed(c)[0]
    } else if vertical {
        c.wm().block_rl()
    } else {
        parent == 3
    };
    let mut bits = u8::from(rl);
    if parent >= 2 && !c.cb_wm.sideways() && !c.wm().text_sideways() {
        bits |= 2;
    }
    if vertical {
        bits |= 4;
    } else if parent == 1 {
        bits |= 8;
    }
    Some(bits)
}

pub(crate) fn project_wrapper(style: &mut gpui::StyleRefinement, c: &Computed) {
    style.baseline_x_flags = baseline_x_flags(c);
    if grid_mode(c) == 0 {
        return;
    }
    style.align_self = c
        .align_self
        .map(|a| super::self_align(a, c.align_self_last));
    style.justify_self = c
        .justify_self
        .map(|a| super::self_align(a, c.justify_self_last));
    if c.align_self_safe || c.justify_self_safe {
        style.safe_alignment = Some((false, c.align_self_safe, false, false));
        style.safe_justify_alignment = Some((false, c.justify_self_safe));
    }
    super::alignment_axes::abspos_normal(style, c);
    super::alignment_axes::grid_self(style, c);
    super::alignment_axes::project(style, false, grid_mode(c) >= 2);
    super::grid_content_baseline::apply(style, c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{Align, BoxStyle, Display};

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
                    display: Some(Display::Flex),
                    ..Computed::default()
                };
                with_inherited.writing_mode = Some(
                    crate::style::computed::writing_mode::WritingModeKw::new(true, rl, sideways),
                );
                with_inherited
            };
            let effective = crate::style::cascade::inherit::inherit(
                &BoxStyle::from(parent.clone()),
                &Computed::default(),
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
                    display: Some(Display::Flex),
                    ..Computed::default()
                };
                with_inherited.writing_mode = Some(
                    crate::style::computed::writing_mode::WritingModeKw::new(true, rl, sideways),
                );
                with_inherited
            };
            let own = {
                let mut with_inherited = Computed {
                    ..Computed::default()
                };
                with_inherited.writing_mode = Some(
                    crate::style::computed::writing_mode::WritingModeKw::new(false, false, false),
                );
                with_inherited.text_orientation =
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
            display: Some(Display::Flex),
            ..Computed::default()
        };
        for (rl, flags) in [(false, 4), (true, 5)] {
            let own = {
                let mut with_inherited = Computed {
                    ..Computed::default()
                };
                with_inherited.writing_mode = Some(
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
                &Computed::default(),
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
            display: Some(Display::GridLanes),
            ..Computed::default()
        };
        let own = {
            let mut with_inherited = Computed {
                justify_self: Some(Align::Baseline),
                justify_self_last: true,
                ..Computed::default()
            };
            with_inherited.writing_mode = Some(
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
        let c = Computed {
            parent_grid: 2,
            align_self: Some(Align::Baseline),
            align_self_last: true,
            align_self_safe: true,
            ..Computed::default()
        };
        let mut wrapper = gpui::StyleRefinement::default();
        project_wrapper(&mut wrapper, &c);
        assert_eq!(wrapper.justify_self, Some(gpui::AlignItems::LastBaseline));
        assert_eq!(wrapper.safe_justify_alignment, Some((false, true)));
    }
}
