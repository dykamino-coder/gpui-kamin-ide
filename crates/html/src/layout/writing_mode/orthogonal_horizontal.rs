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
    if !matches!(child.style.used().display, None | Some(Display::Block))
        || !matches!(child.style.used().width, None | Some(Len::Auto))
    {
        return false;
    }
    let original = child.style.used().width;
    child.style.used_mut().width = Some(Len::FitContent);
    if !native_intrinsic::eligible(child) {
        child.style.used_mut().width = original;
        return false;
    }
    let fallback = available(
        orthogonal_inline::axis(container, false),
        container.orthogonal_scrollport.map(|axes| axes[0]),
        icb_width,
    );
    let px = |value| px_lossy_zero(value);
    let outer = fallback - px(child.style.used().margin.left) - px(child.style.used().margin.right);
    let adjustment = if child.style.used().border_box == Some(true) {
        0.0
    } else {
        orthogonal_inline::edges(&child.style.used(), false)
    };
    // The native functional fit-content clamp preserves min-content when a
    // word is wider than the available space. CSS author limits stay intact.
    child.style.used_mut().fit_arg[0] = Some(Len::Px((outer - adjustment).max(0.0)));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{Display, Position};

    #[test]
    fn omitted_and_authored_auto_have_the_same_intrinsic_contract() {
        let container = Computed {
            width: Some(Len::Px(400.0)),
            ..Computed::default()
        };
        for authored in [None, Some(Len::Auto)] {
            let mut child = crate::layout::table::anon::anon_element("div", vec![]);
            child.style.used_mut().width = authored;
            child.style.used_mut().border_width.left = Some(Len::Px(3.0));
            child.style.used_mut().border_width.right = Some(Len::Px(3.0));
            child.style.used_mut().border_visible = [Some(true); 4];
            child.style.used_mut().max_width = Some(Len::Px(500.0));
            child.style.used_mut().min_width = Some(Len::Px(10.0));
            assert!(size_auto(&mut child, &container, 800.0));
            assert_eq!(child.style.used().width, Some(Len::FitContent));
            assert_eq!(child.style.used().fit_arg[0], Some(Len::Px(394.0)));
            assert_eq!(child.style.used().max_width, Some(Len::Px(500.0)));
            assert_eq!(child.style.used().min_width, Some(Len::Px(10.0)));
        }
    }

    #[test]
    fn nearest_scrollport_and_border_box_define_the_available_border_edge() {
        let mut container = Computed::default();
        container.orthogonal_scrollport = Some([
            crate::style::computed::orthogonal::AxisSizes {
                size: Some(300.0),
                ..Default::default()
            },
            Default::default(),
        ]);
        let mut child = crate::layout::table::anon::anon_element("div", vec![]);
        child.style.used_mut().margin.left = Some(Len::Px(10.0));
        child.style.used_mut().padding.left = Some(Len::Px(20.0));
        child.style.used_mut().border_box = Some(true);
        assert!(size_auto(&mut child, &container, 800.0));
        assert_eq!(child.style.used().fit_arg[0], Some(Len::Px(290.0)));
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
            child.style.used_mut().display = Some(display);
            assert!(!size_auto(&mut child, &container, 800.0));
            assert_eq!(child.style.used().width, None);
        }
        child.style.used_mut().display = Some(Display::Block);
        child.style.used_mut().width = Some(Len::Px(100.0));
        assert!(!size_auto(&mut child, &container, 800.0));
        child.style.used_mut().width = None;
        child.style.used_mut().position = Some(Position::Absolute);
        assert!(!size_auto(&mut child, &container, 800.0));
        child.style.used_mut().position = None;
        child.tag = "img".into();
        assert!(!size_auto(&mut child, &container, 800.0));
    }
}
