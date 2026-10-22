//! KaminIDE patch: рост flex-линии не меняет завершённые элементы и OOF static position.
use crate::{DetailedLayoutInfo, Dimension, FlexWrap, Position, Style, TaffyTree};
use crate::geometry::Size;
use crate::style_helpers::TaffyMaxContent;

#[test]
fn consumed_fragmentainer_space_grows_line_and_preserves_items() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let style = Style { size: Size::from_lengths(40.0, 30.0), flex_shrink: 0.0, ..Style::DEFAULT };
    let a = tree.new_leaf(style.clone()).unwrap();
    let b = tree.new_leaf(style).unwrap();
    let abs = tree.new_leaf(Style { position: Position::Absolute, size: Size::from_lengths(10.0, 10.0), ..Style::DEFAULT }).unwrap();
    let root = tree.new_with_children(Style { size: Size { width: Dimension::length(40.0), height: Dimension::auto() }, flex_wrap: FlexWrap::Wrap, ..Style::DEFAULT }, &[a, b, abs]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let static_position = tree.layout(abs).unwrap().location;
    tree.set_flex_line_fragmentation_growth(root, vec![70.0]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(root).unwrap().size.height, 130.0);
    assert_eq!(tree.layout(a).unwrap().size.height, 30.0);
    assert_eq!(tree.layout(b).unwrap().location.y, 100.0);
    assert_eq!(tree.layout(abs).unwrap().location, static_position);
    let DetailedLayoutInfo::Flex(info) = tree.detailed_layout_info(root) else { panic!("no lines") };
    assert_eq!(info.lines[0].cross_size, 100.0);
    assert_eq!(info.lines[1].cross_offset, 100.0);
    assert_eq!(info.lines[0].items.len(), 1);
    tree.set_flex_line_fragmentation_growth(root, Vec::new()).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(root).unwrap().size.height, 60.0);
    assert!(tree.set_flex_line_fragmentation_growth(root, vec![f32::NAN]).is_err());
}

#[test]
fn column_break_offset_does_not_change_flexible_lengths() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let style = Style { size: Size::from_lengths(40.0, 30.0), flex_grow: 1.0, ..Style::DEFAULT };
    let a = tree.new_leaf(style.clone()).unwrap();
    let b = tree.new_leaf(style).unwrap();
    let root = tree.new_with_children(Style { flex_direction: crate::FlexDirection::Column,
        size: Size::from_lengths(40.0, 100.0), ..Style::DEFAULT }, &[a, b]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(a).unwrap().size.height, 50.0);
    tree.set_flex_item_fragmentation_offset(b, 50.0).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(a).unwrap().size.height, 50.0);
    assert_eq!(tree.layout(b).unwrap().size.height, 50.0);
    assert_eq!(tree.layout(b).unwrap().location.y, 100.0);
    assert_eq!(tree.layout(root).unwrap().size.height, 100.0);
    tree.set_style(root, Style { flex_direction: crate::FlexDirection::Column,
        size: Size { width: Dimension::length(40.0), height: Dimension::auto() }, ..Style::DEFAULT }).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(root).unwrap().size.height, 110.0);
    assert_eq!(tree.layout(b).unwrap().size.height, 30.0);
    assert!(tree.set_flex_item_fragmentation_offset(a, -1.0).is_err());
}
