//! KaminIDE patch: закреплённые линии flex задаются индексами `DetailedFlexInfo`.
use crate::geometry::Size;
use crate::style_helpers::TaffyMaxContent;
use crate::{DetailedLayoutInfo, FlexDirection, FlexWrap, NodeId, Style, TaffyTree};

fn tree(direction: FlexDirection, wrap: FlexWrap) -> (TaffyTree<()>, NodeId, Vec<NodeId>) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let items: Vec<_> = (0..3)
        .map(|_| tree.new_leaf(Style { size: Size::from_lengths(40.0, 40.0), flex_shrink: 0.0, ..Style::DEFAULT }).unwrap())
        .collect();
    let size = if direction.is_row() { Size::from_lengths(100.0, 20.0) } else { Size::from_lengths(20.0, 100.0) };
    let root = tree.new_with_children(Style { flex_direction: direction, flex_wrap: wrap, size, ..Style::DEFAULT }, &items).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    (tree, root, items)
}

fn lines(tree: &TaffyTree<()>, root: NodeId, items: &[NodeId]) -> Vec<usize> {
    let DetailedLayoutInfo::Flex(info) = tree.detailed_layout_info(root) else { panic!("no lines") };
    items.iter().map(|id| info.lines.iter().position(|l| l.items.iter().any(|i| i.node == *id)).unwrap()).collect()
}

#[test]
fn metadata_indices_pin_lines_in_both_wrap_directions() {
    for direction in [FlexDirection::Row, FlexDirection::RowReverse, FlexDirection::Column, FlexDirection::ColumnReverse] {
        for wrap in [FlexWrap::Wrap, FlexWrap::WrapReverse] {
            let (mut tree, root, items) = tree(direction, wrap);
            let natural = lines(&tree, root, &items);
            let layouts: Vec<_> = items.iter().map(|id| *tree.layout(*id).unwrap()).collect();
            for (id, line) in items.iter().zip(&natural) {
                tree.set_flex_item_fragmentation_line(*id, Some(*line)).unwrap();
            }
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            // Pinning the natural lines is not a layout change.
            assert_eq!(items.iter().map(|id| *tree.layout(*id).unwrap()).collect::<Vec<_>>(), layouts);
            // Growth imposed by fragmentation keeps the first two items together.
            let grown = if direction.is_row() { Size::from_lengths(70.0, 40.0) } else { Size::from_lengths(40.0, 70.0) };
            tree.set_style(items[0], Style { size: grown, flex_shrink: 0.0, ..Style::DEFAULT }).unwrap();
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            assert_eq!(lines(&tree, root, &items), natural, "{direction:?} {wrap:?}");
        }
    }
}

#[test]
fn incomplete_or_gapped_pins_are_recollected() {
    for wrap in [FlexWrap::Wrap, FlexWrap::WrapReverse] {
        let (mut tree, root, items) = tree(FlexDirection::Column, wrap);
        let natural = lines(&tree, root, &items);
        tree.set_style(items[0], Style { size: Size::from_lengths(40.0, 70.0), flex_shrink: 0.0, ..Style::DEFAULT }).unwrap();
        let skipped: Vec<_> = natural.iter().map(|&l| if l == natural[2] { l + 2 } else { l }).collect();
        for pins in [vec![Some(natural[0]), Some(natural[1]), None], skipped.into_iter().map(Some).collect()] {
            for (id, line) in items.iter().zip(pins) {
                tree.set_flex_item_fragmentation_line(*id, line).unwrap();
            }
            tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
            let actual = lines(&tree, root, &items);
            assert_ne!(actual[0], actual[1], "{wrap:?}");
        }
    }
}
