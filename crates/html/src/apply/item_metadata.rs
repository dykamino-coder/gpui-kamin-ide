//! Item-facing grid metadata also belongs on an anonymous sizing wrapper.
use crate::computed::Computed;

fn grid_mode(c: &Computed) -> u8 {
    if c.parent_grid != 0 {
        c.parent_grid
    } else if c.parent_lanes {
        match (c.cb_vertical, c.cb_vertical_rl) {
            (false, _) => 1,
            (true, false) => 2,
            (true, true) => 3,
        }
    } else {
        0
    }
}

pub(crate) fn baseline_x_flags(c: &Computed) -> Option<u8> {
    let parent = grid_mode(c);
    if parent == 0 {
        if !c.parent_flex_grid {
            return None;
        }
        let central = c.cb_vertical && !c.cb_sideways && c.text_sideways != Some(true);
        if c.vertical == Some(true) {
            return Some(4 | u8::from(c.vertical_rl == Some(true)) | if central { 2 } else { 0 });
        }
        return c.cb_vertical.then(|| {
            u8::from(c.cb_vertical_rl)
                | if central {
                    2
                } else {
                    0
                }
        });
    }
    let vertical = c.vertical == Some(true);
    let rl = if vertical {
        c.vertical_rl == Some(true)
    } else {
        parent == 3
    };
    let mut bits = u8::from(rl);
    if parent >= 2 && !c.cb_sideways && c.text_sideways != Some(true) {
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
    super::alignment_axes::project(style, false, grid_mode(c) >= 2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computed::{Align, Display};

    #[test]
    fn own_vertical_flex_items_keep_the_parent_central_synthesis_policy() {
        for (rl, sideways, flags) in [(false, false, 6), (true, false, 7),
            (false, true, 4), (true, true, 5)] {
            let parent = Computed {
                display: Some(Display::Flex), vertical: Some(true),
                vertical_rl: Some(rl), sideways: Some(sideways),
                ..Computed::default()
            };
            let effective = crate::inline::inherit(&parent, &Computed::default());
            assert_eq!(baseline_x_flags(&effective), Some(flags));
            let mut wrapper = gpui::StyleRefinement::default();
            project_wrapper(&mut wrapper, &effective);
            assert_eq!(wrapper.baseline_x_flags, Some(flags));
        }
    }

    #[test]
    fn horizontal_flex_items_inherit_vertical_parent_baseline_synthesis() {
        for (rl, sideways, flags) in [
            (false, false, 2),
            (true, false, 3),
            (false, true, 0),
            (true, true, 1),
        ] {
            let parent = Computed {
                display: Some(Display::Flex),
                vertical: Some(true),
                vertical_rl: Some(rl),
                sideways: Some(sideways),
                ..Computed::default()
            };
            let own = Computed {
                vertical: Some(false),
                text_sideways: Some(false),
                ..Computed::default()
            };
            let effective = crate::inline::inherit(&parent, &own);
            assert_eq!(baseline_x_flags(&effective), Some(flags));
            let mut wrapper = gpui::StyleRefinement::default();
            project_wrapper(&mut wrapper, &effective);
            assert_eq!(wrapper.baseline_x_flags, Some(flags));
        }
    }

    #[test]
    fn flex_items_and_wrappers_preserve_own_vertical_baseline_orientation() {
        let parent = Computed {
            display: Some(Display::Flex),
            ..Computed::default()
        };
        for (rl, flags) in [(false, 4), (true, 5)] {
            let own = Computed {
                vertical: Some(true),
                vertical_rl: Some(rl),
                ..Computed::default()
            };
            let effective = crate::inline::inherit(&parent, &own);
            assert_eq!(baseline_x_flags(&effective), Some(flags));
            let mut wrapper = gpui::StyleRefinement::default();
            wrapper.align_self = Some(gpui::AlignItems::End);
            project_wrapper(&mut wrapper, &effective);
            assert_eq!(wrapper.baseline_x_flags, Some(flags));
            assert_eq!(wrapper.align_self, Some(gpui::AlignItems::End));
        }
        assert_eq!(
            baseline_x_flags(&crate::inline::inherit(&parent, &Computed::default())),
            None
        );
    }

    #[test]
    fn lanes_wrapper_retains_inherited_item_orientation() {
        let parent = Computed {
            display: Some(Display::GridLanes),
            ..Computed::default()
        };
        let own = Computed {
            vertical: Some(true),
            vertical_rl: Some(true),
            justify_self: Some(Align::Baseline),
            justify_self_last: true,
            ..Computed::default()
        };
        let effective = crate::inline::inherit(&parent, &own);
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
