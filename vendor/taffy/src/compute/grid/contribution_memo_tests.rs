//! A pass over a large grid lays out the same with and without the contribution memo.
use crate::compute_leaf_layout;
use crate::prelude::*;

/// Rows of the test grid: three items each, so the grid keeps a memo.
const ROWS: usize = 30;

/// Lay out a flex row holding a flex column holding a grid of text items, with the memo on or off.
/// Returns every node's layout bits and the number of contributions read from the memo.
fn lay_out(memo: bool, rows: &dyn Fn(&mut Style)) -> (Vec<[u32; 4]>, u64) {
    let mut tree: TaffyTree<usize> = TaffyTree::new();
    tree.disable_rounding();
    tree.set_contribution_memo(memo);
    let items: Vec<NodeId> = (0..ROWS * 3)
        .map(|index| {
            tree.new_leaf_with_context(Style::DEFAULT, 3 + index % 7 * 4).unwrap()
        })
        .collect();
    let mut style = Style {
        display: Display::Grid,
        grid_template_columns: vec![auto(), fr(1.0), minmax(length(40.0), max_content())],
        ..Style::DEFAULT
    };
    rows(&mut style);
    let grid = tree.new_with_children(style, &items).unwrap();
    let column = tree
        .new_with_children(Style { display: Display::Flex, flex_direction: FlexDirection::Column, ..Style::DEFAULT }, &[grid])
        .unwrap();
    let root = tree
        .new_with_children(
            Style { display: Display::Flex, size: Size { width: length(800.0), height: Dimension::auto() }, ..Style::DEFAULT },
            &[column],
        )
        .unwrap();
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, chars, style| {
        let chars = chars.map_or(0, |chars| *chars) as f32;
        compute_leaf_layout(input, style, |_, _| 0.0, |known, available| {
            // Words of 30px; a line holds as many as the width allows.
            let widest = chars * 10.0;
            let width = known.width.unwrap_or(match available.width {
                AvailableSpace::Definite(width) => width.clamp(30.0, widest),
                AvailableSpace::MinContent => 30.0,
                AvailableSpace::MaxContent => widest,
            });
            let lines = (widest / width.max(1.0)).ceil();
            Size { width, height: known.height.unwrap_or(lines * 16.0) }
        })
    })
    .unwrap();
    let layout = |node: NodeId| {
        let layout = tree.layout(node).unwrap();
        [layout.location.x, layout.location.y, layout.size.width, layout.size.height].map(f32::to_bits)
    };
    let mut nodes = vec![root, column, grid];
    nodes.extend(&items);
    (nodes.into_iter().map(layout).collect(), tree.contribution_memo_stats().hits)
}

/// Same layout with and without the memo, which answers some of the contributions.
fn assert_transparent(rows: &dyn Fn(&mut Style)) {
    let (with_memo, hits) = lay_out(true, rows);
    let (without_memo, no_hits) = lay_out(false, rows);
    assert_eq!(with_memo, without_memo);
    assert_eq!(no_hits, 0);
    assert!(hits > 0);
}

#[test]
fn memo_keeps_layout_with_auto_rows() {
    assert_transparent(&|style| style.grid_auto_rows = vec![auto()]);
}

#[test]
fn memo_keeps_layout_with_flexible_rows() {
    assert_transparent(&|style| {
        style.grid_template_rows = vec![fr(1.0), fr(2.0), auto()];
        style.grid_auto_rows = vec![fr(1.0)];
    });
}

#[test]
fn memo_keeps_layout_when_base_size_and_growth_limit_differ() {
    assert_transparent(&|style| {
        style.grid_template_rows = vec![minmax(length(10.0), max_content())];
        style.grid_auto_rows = vec![minmax(length(20.0), auto()), fit_content(length(30.0))];
    });
}