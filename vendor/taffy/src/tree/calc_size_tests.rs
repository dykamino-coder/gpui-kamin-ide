//! calc-size preserves intrinsic contributions, automatic minima and its sizing box.

use super::*;
use crate::LengthPercentage;

fn input(available: AvailableSpace) -> LayoutInput {
    LayoutInput {
        run_mode: RunMode::ComputeSize,
        available_space: Size { width: available, height: AvailableSpace::MaxContent },
        ..LayoutInput::HIDDEN
    }
}

fn measured(inputs: LayoutInput, _: NodeId, _: Option<&mut ()>, style: &Style) -> LayoutOutput {
    compute_leaf_layout(inputs, style, calc_value, |known, available| Size {
        width: known.width.unwrap_or(match available.width {
            AvailableSpace::MinContent => 20.0,
            AvailableSpace::MaxContent => 40.0,
            AvailableSpace::Definite(value) => value.clamp(20.0, 40.0),
        }),
        height: known.height.unwrap_or(10.0),
    })
}

fn width(style: Style, inputs: LayoutInput) -> f32 {
    let mut tree = TaffyTree::<()>::new();
    let node = tree.new_leaf(style).unwrap();
    let mut view = TaffyView { taffy: &mut tree, measure_function: measured };
    view.compute_child_layout(node, inputs, #[cfg(feature = "block_layout")] None).size.width
}

#[test]
fn auto_basis_keeps_distinct_intrinsic_contributions() {
    let mut style = Style::DEFAULT;
    style.calc_size[0] = Some((1.0, 80.0, f32::MAX, f32::MIN));
    assert_eq!(width(style.clone(), input(AvailableSpace::MinContent)), 100.0);
    assert_eq!(width(style, input(AvailableSpace::MaxContent)), 120.0);
}

#[test]
fn explicit_basis_and_fit_content_use_the_requested_constraint() {
    let mut style = Style::DEFAULT;
    style.calc_size[0] = Some((1.0, 80.0, f32::MAX, f32::MIN));
    style.calc_size_basis[0] = Dimension::min_content();
    assert_eq!(width(style.clone(), input(AvailableSpace::MaxContent)), 100.0);
    style.calc_size_basis[0] = Dimension::max_content();
    assert_eq!(width(style.clone(), input(AvailableSpace::MinContent)), 120.0);
    style.calc_size_basis[0] = Dimension::fit_content();
    style.margin.left = crate::LengthPercentageAuto::length(15.0);
    assert_eq!(width(style.clone(), input(AvailableSpace::Definite(30.0))), 110.0);
    assert_eq!(width(style, input(AvailableSpace::Definite(0.0))), 100.0);
}

#[test]
fn calculation_uses_content_box_before_adding_edges() {
    let mut style = Style::DEFAULT;
    style.calc_size[0] = Some((2.0, 0.0, f32::MAX, f32::MIN));
    style.calc_size_content_box = true;
    style.padding.left = LengthPercentage::length(10.0);
    assert_eq!(width(style.clone(), input(AvailableSpace::MinContent)), 50.0);
    assert_eq!(width(style, input(AvailableSpace::MaxContent)), 90.0);
}

#[test]
fn ordinary_auto_minimum_is_zero() {
    let mut style = Style::DEFAULT;
    style.calc_size[2] = Some((1.0, 10.0, f32::MAX, f32::MIN));
    let mut inputs = input(AvailableSpace::Definite(100.0));
    inputs.known_dimensions.width = Some(15.0);
    assert_eq!(width(style, inputs), 15.0);
}

#[test]
fn explicit_maximum_and_minimum_clamp_in_css_order() {
    let mut style = Style::DEFAULT;
    style.calc_size_max[0] = Some((1.0, -10.0, f32::MAX, f32::MIN));
    style.calc_size_basis[4] = Dimension::max_content();
    let mut inputs = input(AvailableSpace::MaxContent);
    inputs.known_dimensions.width = Some(100.0);
    assert_eq!(width(style.clone(), inputs), 30.0);
    style.calc_size[2] = Some((1.0, 20.0, f32::MAX, f32::MIN));
    style.calc_size_basis[2] = Dimension::min_content();
    assert_eq!(width(style, inputs), 40.0);
}

#[cfg(feature = "block_layout")]
#[test]
fn automatic_minimum_uses_content_before_ratio_transfer() {
    for (axis, ratio) in [(AbsoluteAxis::Horizontal, 0.5), (AbsoluteAxis::Vertical, 2.0)] {
        let mut tree = TaffyTree::<()>::new();
        let mut child = Style::DEFAULT;
        child.size = Size::ZERO.map(Dimension::length);
        child.size.set_abs(axis, Dimension::length(150.0));
        let child = tree.new_leaf(child).unwrap();
        let mut style = Style::DEFAULT;
        style.display = Display::Block;
        style.aspect_ratio = Some(ratio);
        style.size.set_abs(axis.other_axis(), Dimension::length(100.0));
        let index = if axis == AbsoluteAxis::Horizontal { 2 } else { 3 };
        style.calc_size[index] = Some((1.0, -50.0, f32::MAX, f32::MIN));
        let node = tree.new_with_children(style, &[child]).unwrap();
        let mut inputs = input(AvailableSpace::Definite(800.0));
        inputs.known_dimensions.set_abs(axis, None);
        inputs.known_dimensions.set_abs(axis.other_axis(), Some(100.0));
        let mut view = TaffyView { taffy: &mut tree, measure_function: measured };
        let output = view.compute_child_layout(node, inputs, None);
        assert_eq!(output.size.get_abs(axis), 100.0);
    }
}

#[cfg(feature = "flexbox")]
#[test]
fn intrinsic_calc_size_survives_column_fit_content_parent() {
    let mut tree = TaffyTree::<()>::new();
    let mut leaf = Style::DEFAULT;
    leaf.calc_size[0] = Some((1.0, 80.0, f32::MAX, f32::MIN));
    let leaf = tree.new_leaf(leaf).unwrap();
    let mut fit = Style::DEFAULT;
    fit.display = Display::Flex;
    fit.flex_direction = crate::FlexDirection::Column;
    fit.size = Size { width: Dimension::fit_content(), height: Dimension::length(100.0) };
    let fit = tree.new_with_children(fit, &[leaf]).unwrap();
    let mut outer = Style::DEFAULT;
    outer.display = Display::Flex;
    outer.flex_direction = crate::FlexDirection::Column;
    outer.size.width = Dimension::length(0.0);
    let outer = tree.new_with_children(outer, &[fit]).unwrap();
    tree.compute_layout_with_measure(outer, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent }, measured).unwrap();
    assert_eq!(tree.layout(fit).unwrap().size.width, 100.0);
}

#[cfg(feature = "flexbox")]
#[test]
fn fit_content_preserves_ratio_transferred_cross_size() {
    let mut tree = TaffyTree::<()>::new();
    let mut child = Style::DEFAULT;
    child.display = Display::Flex;
    child.flex_direction = crate::FlexDirection::Column;
    child.aspect_ratio = Some(2.0);
    child.size = Size { width: Dimension::fit_content(), height: Dimension::length(50.0) };
    let child = tree.new_leaf(child).unwrap();
    let mut outer = Style::DEFAULT;
    outer.display = Display::Flex;
    outer.flex_direction = crate::FlexDirection::Column;
    outer.size.width = Dimension::length(800.0);
    let outer = tree.new_with_children(outer, &[child]).unwrap();
    tree.compute_layout(outer, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent }).unwrap();
    assert_eq!(tree.layout(child).unwrap().size.width, 100.0);
    assert_eq!(tree.layout(child).unwrap().size.height, 50.0);
}

#[cfg(feature = "flexbox")]
#[test]
fn fit_content_respects_transferred_cross_minimum() {
    let mut tree = TaffyTree::<()>::new();
    let mut child = Style::DEFAULT;
    child.display = Display::Flex;
    child.flex_direction = crate::FlexDirection::Column;
    child.min_size.width = crate::LengthPercentageAuto::length(100.0);
    child.size = Size { width: Dimension::fit_content(), height: Dimension::length(50.0) };
    let child = tree.new_leaf(child).unwrap();
    let mut outer = Style::DEFAULT;
    outer.display = Display::Flex;
    outer.flex_direction = crate::FlexDirection::Column;
    outer.size.width = Dimension::length(800.0);
    let outer = tree.new_with_children(outer, &[child]).unwrap();
    tree.compute_layout(outer, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent }).unwrap();
    assert_eq!(tree.layout(child).unwrap().size.width, 100.0);
    assert_eq!(tree.layout(child).unwrap().size.height, 50.0);
}
