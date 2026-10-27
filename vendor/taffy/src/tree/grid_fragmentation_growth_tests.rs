//! KaminIDE patch: fragmentation growth after grid tracks keeps the unfragmented sizing.
use crate::geometry::AbsoluteAxis;
use crate::prelude::*;
use crate::{AlignContent, DetailedLayoutInfo};

fn rows(start: i16, end: i16) -> Line<GridPlacement> {
    Line { start: line(start), end: line(end) }
}

fn item(tree: &mut TaffyTree<()>, height: Option<f32>, start: i16, end: i16) -> NodeId {
    let size = Size { width: Dimension::auto(), height: height.map_or(Dimension::auto(), Dimension::length) };
    tree.new_leaf(Style { size, grid_row: rows(start, end), ..Style::DEFAULT }).unwrap()
}

fn grid(tree: &mut TaffyTree<()>, style: Style, children: &[NodeId]) -> NodeId {
    let style = Style {
        display: Display::Grid,
        grid_template_columns: vec![length(20.0)],
        grid_template_rows: vec![auto(), auto()],
        ..style
    };
    tree.new_with_children(style, children).unwrap()
}

fn row_positions(tree: &TaffyTree<()>, node: NodeId) -> Vec<(f32, f32)> {
    let DetailedLayoutInfo::Grid(info) = tree.detailed_layout_info(node) else { panic!("no grid info") };
    info.rows.positions.iter().map(|l| (l.start, l.end)).collect()
}

fn frame(tree: &TaffyTree<()>, node: NodeId) -> (f32, f32) {
    let layout = tree.layout(node).unwrap();
    (layout.location.y, layout.size.height)
}

#[test]
fn zero_growth_is_the_unfragmented_layout() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let items = [item(&mut tree, Some(50.0), 1, 2), item(&mut tree, Some(40.0), 2, 3)];
    let style = Style { size: Size { width: auto(), height: length(150.0) }, align_content: AlignContent::CENTER, ..Style::DEFAULT };
    let root = grid(&mut tree, style, &items);
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    let unfragmented: Vec<_> = items.iter().map(|&id| frame(&tree, id)).collect();
    let tracks = row_positions(&tree, root);
    for growth in [vec![0.0, 0.0], vec![]] {
        tree.set_grid_fragmentation_growth(root, AbsoluteAxis::Vertical, growth).unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(items.iter().map(|&id| frame(&tree, id)).collect::<Vec<_>>(), unfragmented);
        assert_eq!(row_positions(&tree, root), tracks);
    }
    assert!(tree.set_grid_fragmentation_growth(root, AbsoluteAxis::Vertical, vec![-1.0]).is_err());
}

#[test]
fn growth_moves_following_rows_and_spanning_areas() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let first = item(&mut tree, Some(50.0), 1, 2);
    let stretched = item(&mut tree, None, 1, 2);
    let second = item(&mut tree, Some(40.0), 2, 3);
    let spanning = item(&mut tree, None, 1, 3);
    let root = grid(&mut tree, Style::DEFAULT, &[first, stretched, second, spanning]);
    tree.set_grid_fragmentation_growth(root, AbsoluteAxis::Vertical, vec![30.0]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    // Completed items of the first row keep their used size; the pushed row moves.
    assert_eq!(frame(&tree, first), (0.0, 50.0));
    assert_eq!(frame(&tree, stretched), (0.0, 50.0));
    assert_eq!(frame(&tree, second), (80.0, 40.0));
    assert_eq!(frame(&tree, spanning), (0.0, 120.0));
    assert_eq!(tree.layout(root).unwrap().size.height, 120.0);
    assert_eq!(row_positions(&tree, root), vec![(0.0, 50.0), (80.0, 120.0)]);
}

#[test]
fn definite_and_clamped_containers_keep_their_size() {
    for (style, expected_rows, height) in [
        (
            Style { size: Size { width: auto(), height: length(100.0) }, align_content: AlignContent::END, ..Style::DEFAULT },
            vec![(10.0, 60.0), (90.0, 130.0)],
            100.0,
        ),
        (Style { max_size: Size { width: auto(), height: length(100.0) }, ..Style::DEFAULT }, vec![(0.0, 50.0), (80.0, 120.0)], 100.0),
    ] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let items = [item(&mut tree, Some(50.0), 1, 2), item(&mut tree, Some(40.0), 2, 3)];
        let root = grid(&mut tree, style, &items);
        tree.set_grid_fragmentation_growth(root, AbsoluteAxis::Vertical, vec![30.0]).unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        // The free space of the unfragmented layout is kept; content overflows a fixed size.
        assert_eq!(row_positions(&tree, root), expected_rows);
        assert_eq!(tree.layout(root).unwrap().size.height, height);
    }
}

#[test]
fn reversed_axis_keeps_completed_rows_at_its_start_edge() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let items = [item(&mut tree, Some(50.0), 1, 2), item(&mut tree, Some(40.0), 2, 3)];
    let style = Style { grid_axis_reversed: Some(Size { width: false, height: true }), ..Style::DEFAULT };
    let root = grid(&mut tree, style, &items);
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(row_positions(&tree, root), vec![(40.0, 90.0), (0.0, 40.0)]);
    tree.set_grid_fragmentation_growth(root, AbsoluteAxis::Vertical, vec![30.0]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(row_positions(&tree, root), vec![(70.0, 120.0), (0.0, 40.0)]);
    assert_eq!(frame(&tree, items[0]), (70.0, 50.0));
    assert_eq!(tree.layout(root).unwrap().size.height, 120.0);
}

#[test]
fn horizontal_block_axis_grows_columns_only() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let column = |tree: &mut TaffyTree<()>, width: f32, start: i16| {
        let size = Size { width: length(width), height: auto() };
        tree.new_leaf(Style { size, grid_column: rows(start, start + 1), ..Style::DEFAULT }).unwrap()
    };
    let items = [column(&mut tree, 50.0, 1), column(&mut tree, 40.0, 2)];
    let style = Style {
        display: Display::Grid,
        grid_template_columns: vec![auto(), auto()],
        grid_template_rows: vec![length(20.0)],
        ..Style::DEFAULT
    };
    let root = tree.new_with_children(style, &items).unwrap();
    tree.set_grid_fragmentation_growth(root, AbsoluteAxis::Horizontal, vec![30.0]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(tree.layout(items[1]).unwrap().location.x, 80.0);
    assert_eq!(tree.layout(root).unwrap().size, Size { width: 120.0, height: 20.0 });
}

#[test]
fn subgridded_rows_follow_the_parent_growth_only() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let inner = [item(&mut tree, Some(50.0), 1, 2), item(&mut tree, Some(40.0), 2, 3)];
    let subgrid = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                subgrid: crate::style::SUBGRID_ROWS,
                grid_template_columns: vec![length(20.0)],
                grid_row: rows(1, 3),
                ..Style::DEFAULT
            },
            &inner,
        )
        .unwrap();
    let root = grid(&mut tree, Style::DEFAULT, &[subgrid]);
    tree.set_grid_fragmentation_growth(subgrid, AbsoluteAxis::Vertical, vec![30.0]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    // The subgrid has no own row tracks: its request is ignored.
    assert_eq!(frame(&tree, inner[1]), (50.0, 40.0));
    tree.set_grid_fragmentation_growth(subgrid, AbsoluteAxis::Vertical, vec![]).unwrap();
    tree.set_grid_fragmentation_growth(root, AbsoluteAxis::Vertical, vec![30.0]).unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(frame(&tree, inner[1]), (80.0, 40.0));
    assert_eq!(frame(&tree, subgrid), (0.0, 120.0));
}

#[test]
fn measured_growth_is_not_distributed_by_content_alignment() {
    // The outer row is sized from the inner grid's content including its growth; the stretched
    // inner grid must not hand that growth to its `normal` (stretch) content alignment.
    let mut tree: TaffyTree<()> = TaffyTree::new();
    tree.disable_rounding();
    let items = [item(&mut tree, Some(50.0), 1, 2), item(&mut tree, Some(40.0), 2, 3)];
    let inner = grid(&mut tree, Style::DEFAULT, &items);
    let outer = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                grid_template_columns: vec![auto()],
                grid_template_rows: vec![auto()],
                ..Style::DEFAULT
            },
            &[inner],
        )
        .unwrap();
    tree.set_grid_fragmentation_growth(inner, AbsoluteAxis::Vertical, vec![30.0]).unwrap();
    tree.compute_layout(outer, Size::MAX_CONTENT).unwrap();
    assert_eq!(frame(&tree, inner), (0.0, 120.0));
    assert_eq!(row_positions(&tree, inner), vec![(0.0, 50.0), (80.0, 120.0)]);
}
