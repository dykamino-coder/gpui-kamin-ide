//! Verify CSS values reach real GPUI styles, including projected grid axes.

use super::*;
use crate::style::computed::Computed;
use crate::style::css::parse_decls;
use gpui::{Styled, px, relative};

/// Стиль применяется к настоящему `Div` и читается обратно из `Style` —
/// так проверяется именно маппинг, а не наше представление о нём.
fn styled(css: &str) -> gpui::StyleRefinement {
    let mut c = Computed::default();
    c.apply_decls(&parse_decls(css));
    let mut d = apply(gpui::div(), &c);
    d.style().clone()
}

#[test]
fn box_model_reaches_gpui() {
    let s = styled("padding: 4px 8px; margin-top: 6px; border: 2px solid #333");
    assert_eq!(s.padding.top, Some(px(4.).into()));
    assert_eq!(s.padding.right, Some(px(8.).into()));
    assert_eq!(s.margin.top, Some(px(6.).into()));
    assert_eq!(s.border_widths.top, Some(px(2.).into()));
    assert!(s.border_color.is_some());
}

#[test]
fn flex_layout_reaches_gpui() {
    let s = styled("display: flex; flex-direction: column; align-items: center; gap: 6px");
    assert_eq!(s.display, Some(gpui::Display::Flex));
    assert_eq!(s.flex_direction, Some(gpui::FlexDirection::Column));
    assert_eq!(s.align_items, Some(gpui::AlignItems::Center));
    assert_eq!(s.gap.height, Some(px(6.).into()));
}

#[test]
fn percentage_width_becomes_a_fraction() {
    let s = styled("width: 50%");
    assert_eq!(s.size.width, Some(relative(0.5).into()));
}

#[test]
fn multiple_shadows_survive() {
    let s = styled("box-shadow: 0 1px 2px #000, 0 4px 12px rgba(0,0,0,.5)");
    assert_eq!(s.box_shadow.as_ref().map(Vec::len), Some(2));
}

#[test]
fn gradient_takes_first_and_last_stop() {
    let s = styled("background: linear-gradient(90deg, #000000, #444444, #ffffff)");
    assert!(s.background.is_some(), "градиент доехал до фона");
}

#[test]
fn unsupported_properties_leave_no_trace() {
    // Ни фильтров, ни трансформов, ни z-index в GPUI нет: стиль обязан
    // остаться пустым, а не получить приблизительную замену.
    let s = styled("filter: blur(4px); transform: rotate(45deg); z-index: 5; float: left");
    assert!(s.background.is_none() && s.opacity.is_none());
    assert!(s.size.width.is_none() && s.size.height.is_none());
}

#[test]
fn justify_baselines_preserve_first_last_and_override() {
    let last = styled("display: grid; justify-items: last baseline; justify-self: last baseline");
    assert_eq!(last.justify_items, Some(gpui::AlignItems::LastBaseline));
    assert_eq!(last.justify_self, Some(gpui::AlignItems::LastBaseline));
    let first = styled(
        "display: grid; justify-items: last baseline; justify-items: first baseline; justify-self: last baseline; justify-self: first baseline",
    );
    assert_eq!(first.justify_items, Some(gpui::AlignItems::Baseline));
    assert_eq!(first.justify_self, Some(gpui::AlignItems::Baseline));
}

#[test]
fn vertical_grid_axes_preserve_last_baseline_keywords() {
    let grid = styled(
        "display: grid; writing-mode: vertical-rl; align-items: last baseline; justify-items: center",
    );
    assert_eq!(grid.align_items, Some(gpui::AlignItems::Center));
    assert_eq!(grid.justify_items, Some(gpui::AlignItems::LastBaseline));
    let mut c = Computed::default();
    c.apply_decls(&parse_decls(
        "align-self: last baseline; justify-self: center",
    ));
    c.parent_grid = 2;
    let mut child = apply(gpui::div(), &c);
    assert_eq!(child.style().align_self, Some(gpui::AlignItems::Center));
    assert_eq!(
        child.style().justify_self,
        Some(gpui::AlignItems::LastBaseline)
    );
}

#[test]
fn grid_safety_moves_with_container_and_parent_axes() {
    let grid = styled(
        "display: grid; writing-mode: vertical-rl; align-items: unsafe end; justify-items: safe center; align-content: unsafe end; justify-content: safe center",
    );
    assert_eq!(grid.safe_alignment, Some((true, false, true, false)));
    assert_eq!(grid.safe_justify_alignment, Some((false, false)));
    let mut c = Computed::default();
    c.apply_decls(&parse_decls(
        "align-self: unsafe end; justify-self: safe center",
    ));
    c.parent_grid = 2;
    let mut child = apply(gpui::div(), &c);
    assert_eq!(
        child.style().safe_alignment,
        Some((false, true, false, false))
    );
    assert_eq!(child.style().safe_justify_alignment, Some((false, false)));
}

#[test]
fn ratio_preferred_basis_is_distinct_from_auto_used_height() {
    let s = styled("width: 100px; aspect-ratio: 2 / 1");
    assert_eq!(s.aspect_ratio, None);
    assert_eq!(s.aspect_ratio_preferred_size, Some([None, Some(50.0)]));
    assert!(s.size.height.is_none());
    let ordinary = styled("width: 100px; min-height: 50px");
    assert_eq!(ordinary.aspect_ratio_preferred_size, None);
}

#[test]
fn positioned_intrinsic_dimensions_reach_native_layout() {
    let s = styled("position: absolute; width: min-content; height: fit-content");
    assert_eq!(
        s.sizing_keywords,
        Some([
            Some(gpui::CssSizingKeyword::MinContent),
            Some(gpui::CssSizingKeyword::FitContent),
        ])
    );
    let fixed = styled("position: fixed; width: fit-content(40px); height: max-content");
    assert_eq!(
        fixed.sizing_keywords,
        Some([
            Some(gpui::CssSizingKeyword::FitContentPixels(40.0)),
            Some(gpui::CssSizingKeyword::MaxContent),
        ])
    );
}
