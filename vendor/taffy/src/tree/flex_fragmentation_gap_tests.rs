//! KaminIDE patch: отрицательный рост линии гасит промежуток у разрыва (css-gaps-1 §2).
use crate::geometry::Size;
use crate::style::LengthPercentage;
use crate::style_helpers::TaffyMaxContent;
use crate::{AlignContent, DetailedLayoutInfo, Dimension, FlexWrap, NodeId, Style, TaffyTree};

/// Two 50px lines in a 40px wide wrapping row container.
fn rows(container: Style) -> (TaffyTree<()>, NodeId, Vec<NodeId>) {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let items: Vec<_> = (0..2)
        .map(|_| tree.new_leaf(Style { size: Size::from_lengths(40.0, 50.0), flex_shrink: 0.0, ..Style::DEFAULT }).unwrap())
        .collect();
    let root = tree.new_with_children(Style { flex_wrap: FlexWrap::Wrap, ..container }, &items).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    (tree, root, items)
}

fn size(height: Dimension) -> Size<Dimension> {
    Size { width: Dimension::length(40.0), height }
}

fn gap(row: f32) -> Size<LengthPercentage> {
    Size { width: LengthPercentage::length(0.0), height: LengthPercentage::length(row) }
}

fn suppress(tree: &mut TaffyTree<()>, root: NodeId, growth: f32) {
    tree.set_flex_line_fragmentation_growth(root, vec![growth]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
}

#[test]
fn suppressed_gap_shrinks_a_content_sized_container() {
    let (mut tree, root, items) =
        rows(Style { size: size(Dimension::auto()), gap: gap(100.0), align_content: AlignContent::START, ..Style::DEFAULT });
    assert_eq!(tree.layout(items[1]).unwrap().location.y, 150.0);
    suppress(&mut tree, root, -50.0);
    assert_eq!(tree.layout(items[0]).unwrap().location.y, 0.0);
    assert_eq!(tree.layout(items[0]).unwrap().size.height, 50.0);
    assert_eq!(tree.layout(items[1]).unwrap().location.y, 100.0);
    assert_eq!(tree.layout(root).unwrap().size.height, 150.0);
    let DetailedLayoutInfo::Flex(info) = tree.detailed_layout_info(root) else { panic!("no lines") };
    assert_eq!((info.lines[0].cross_size, info.lines[1].cross_offset), (50.0, 100.0));
    suppress(&mut tree, root, 0.0);
    assert_eq!(tree.layout(root).unwrap().size.height, 200.0);
}

#[test]
fn definite_and_minimum_sizes_keep_their_block_size_and_alignment() {
    let definite = Style { size: size(Dimension::length(300.0)), align_content: AlignContent::SPACE_BETWEEN, ..Style::DEFAULT };
    let minimum = Style {
        size: size(Dimension::auto()),
        min_size: size(Dimension::length(300.0)),
        gap: gap(100.0),
        align_content: AlignContent::END,
        ..Style::DEFAULT
    };
    for (style, first) in [(definite, 0.0), (minimum, 100.0)] {
        let (mut tree, root, items) = rows(style);
        assert_eq!(tree.layout(items[0]).unwrap().location.y, first);
        assert_eq!(tree.layout(items[1]).unwrap().location.y, 250.0);
        suppress(&mut tree, root, -60.0);
        assert_eq!(tree.layout(root).unwrap().size.height, 300.0);
        assert_eq!(tree.layout(items[0]).unwrap().location.y, first);
        assert_eq!(tree.layout(items[1]).unwrap().location.y, 190.0);
    }
}
