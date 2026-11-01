//! Авто-ширина ортогонального горизонтального блока через нативный `fit-content(<доступное>)`
//! с внутренним полом `min-content`, а не через искусственный CSS `max-width`: слово шире
//! доступного места не обрезается, авторские пределы остаются нетронутыми.
use crate::dom::Element;
use crate::layout::writing_mode::{native_intrinsic, orthogonal_inline};
use crate::style::computed::orthogonal::available;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::{Len, px_lossy_zero};

pub(super) fn size_auto(child: &mut Element, container: &Computed, icb_width: f32) -> bool {
    // This fallback sizes ordinary blocks; grid and flex own their item sizing.
    if !matches!(
        child.style.used().reset_box_layout.display,
        None | Some(Display::Block)
    ) || !matches!(
        child.style.used().reset_box_layout.width,
        None | Some(Len::Auto)
    ) {
        return false;
    }
    let original = child.style.used().reset_box_layout.width;
    child.style.used_mut().reset_box_layout.width = Some(Len::FitContent);
    if !native_intrinsic::eligible(child) {
        child.style.used_mut().reset_box_layout.width = original;
        return false;
    }
    let fallback = available(
        orthogonal_inline::axis(container, false),
        container
            .reset_context
            .orthogonal_scrollport
            .map(|axes| axes[0]),
        icb_width,
    );
    let px = |value| px_lossy_zero(value);
    let outer = fallback
        - px(child.style.used().reset_box_layout.margin.left)
        - px(child.style.used().reset_box_layout.margin.right);
    let adjustment = if child.style.used().reset_border.border_box == Some(true) {
        0.0
    } else {
        orthogonal_inline::edges(&child.style.used(), false)
    };
    // The native functional fit-content clamp preserves min-content when a
    // word is wider than the available space. CSS author limits stay intact.
    child.style.used_mut().reset_context.fit_arg[0] = Some(Len::Px((outer - adjustment).max(0.0)));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{Display, Position};

    #[test]
    fn omitted_and_authored_auto_have_the_same_intrinsic_contract() {
        let container = Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    width: Some(Len::Px(400.0)),
                    ..Default::default()
                },
            ),
            ..Computed::default()
        };
        for authored in [None, Some(Len::Auto)] {
            let mut child = crate::layout::table::anon::anon_element("div", vec![]);
            child.style.used_mut().reset_box_layout.width = authored;
            child.style.used_mut().reset_border.border_width.left = Some(Len::Px(3.0));
            child.style.used_mut().reset_border.border_width.right = Some(Len::Px(3.0));
            child.style.used_mut().reset_border.border_visible = [Some(true); 4];
            child.style.used_mut().reset_box_layout.max_width = Some(Len::Px(500.0));
            child.style.used_mut().reset_box_layout.min_width = Some(Len::Px(10.0));
            assert!(size_auto(&mut child, &container, 800.0));
            assert_eq!(
                child.style.used().reset_box_layout.width,
                Some(Len::FitContent)
            );
            assert_eq!(
                child.style.used().reset_context.fit_arg[0],
                Some(Len::Px(394.0))
            );
            assert_eq!(
                child.style.used().reset_box_layout.max_width,
                Some(Len::Px(500.0))
            );
            assert_eq!(
                child.style.used().reset_box_layout.min_width,
                Some(Len::Px(10.0))
            );
        }
    }

    #[test]
    fn nearest_scrollport_and_border_box_define_the_available_border_edge() {
        let mut container = Computed::default();
        container.reset_context.orthogonal_scrollport = Some([
            crate::style::computed::orthogonal::AxisSizes {
                size: Some(300.0),
                ..Default::default()
            },
            Default::default(),
        ]);
        let mut child = crate::layout::table::anon::anon_element("div", vec![]);
        child.style.used_mut().reset_box_layout.margin.left = Some(Len::Px(10.0));
        child.style.used_mut().reset_box_layout.padding.left = Some(Len::Px(20.0));
        child.style.used_mut().reset_border.border_box = Some(true);
        assert!(size_auto(&mut child, &container, 800.0));
        assert_eq!(
            child.style.used().reset_context.fit_arg[0],
            Some(Len::Px(290.0))
        );
    }

    #[test]
    fn independent_layout_roles_and_authored_width_keep_their_own_contract() {
        let container = Computed::default();
        let mut child = crate::layout::table::anon::anon_element("div", vec![]);
        for display in [
            Display::Flex,
            Display::Grid,
            Display::InlineGrid,
            Display::GridLanes,
        ] {
            child.style.used_mut().reset_box_layout.display = Some(display);
            assert!(!size_auto(&mut child, &container, 800.0));
            assert_eq!(child.style.used().reset_box_layout.width, None);
        }
        child.style.used_mut().reset_box_layout.display = Some(Display::Block);
        child.style.used_mut().reset_box_layout.width = Some(Len::Px(100.0));
        assert!(!size_auto(&mut child, &container, 800.0));
        child.style.used_mut().reset_box_layout.width = None;
        child.style.used_mut().reset_positioned.position = Some(Position::Absolute);
        assert!(!size_auto(&mut child, &container, 800.0));
        child.style.used_mut().reset_positioned.position = None;
        child.tag = "img".into();
        assert!(!size_auto(&mut child, &container, 800.0));
    }
}
