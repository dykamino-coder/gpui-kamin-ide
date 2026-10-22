//! KaminIDE patch: итоговые baseline X/Y сохраняют порядок, normal-flow и отсутствие каналов.
use crate::prelude::*;
use crate::{Baselines, Contain, GridLanes, MeasureOutput, Overflow, Point};

fn metrics(last_only: bool) -> MeasureOutput {
    MeasureOutput {
        size: Size {
            width: 40.0,
            height: 40.0,
        },
        baseline: (!last_only).then_some(-3.0),
        last_baseline: Some(55.0),
        baseline_x: (!last_only).then_some(55.0),
        last_baseline_x: Some(-3.0),
        baseline_x_from_right: false,
    }
}

fn layout(tree: &mut TaffyTree<bool>, root: NodeId) {
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, context, style| {
        crate::compute_leaf_layout(input, style, |_, _| 0.0, |_, _| metrics(*context.unwrap()))
    })
    .unwrap();
}

fn lane(rows: bool) -> GridLanes {
    GridLanes {
        rows,
        fill_reverse: false,
        track_reverse: false,
        dense: false,
        tolerance: 0.0,
        tolerance_pct: None,
        stack_block: false,
    }
}

#[test]
fn relative_offsets_leave_final_baselines_and_completed_cache_unchanged() {
    for (display, lanes) in [
        (Display::Block, None),
        (Display::Flex, None),
        (Display::Grid, None),
        (Display::Grid, Some(lane(false))),
        (Display::Grid, Some(lane(true))),
    ] {
        for overflow in [Overflow::Visible, Overflow::Hidden] {
            let mut tree = TaffyTree::new();
            tree.disable_rounding();
            let child = tree
                .new_leaf_with_context(
                    Style {
                        size: Size::from_lengths(40.0, 40.0),
                        position: Position::Relative,
                        overflow: Point {
                            x: overflow,
                            y: overflow,
                        },
                        ..Style::DEFAULT
                    },
                    false,
                )
                .unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        display,
                        grid_lanes: lanes,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::START,
                        justify_items: AlignItems::START,
                        size: Size::from_lengths(100.0, 100.0),
                        grid_template_columns: vec![length(80.0)],
                        grid_template_rows: vec![length(80.0)],
                        padding: Rect::length(5.0),
                        border: Rect::length(2.0),
                        ..Style::DEFAULT
                    },
                    &[child],
                )
                .unwrap();
            layout(&mut tree, root);
            let before = tree.computed_layout_output(root).unwrap().unwrap();
            let origin = tree.unrounded_layout(child).location;
            let scroll = overflow == Overflow::Hidden;
            let y = if scroll { (0.0, 40.0) } else { (-3.0, 55.0) };
            let x = if scroll { (40.0, 0.0) } else { (55.0, -3.0) };
            assert_eq!(
                before.baselines,
                Baselines {
                    first: Some(origin.y + y.0),
                    last: Some(origin.y + y.1),
                },
                "{display:?} {lanes:?} {overflow:?}"
            );
            assert_eq!(
                before.baselines_x,
                Baselines {
                    first: Some(origin.x + x.0),
                    last: Some(origin.x + x.1),
                },
                "{display:?} {lanes:?} {overflow:?}"
            );
            let mut style = tree.style(child).unwrap().clone();
            style.inset = Rect {
                left: length(7.0),
                top: length(11.0),
                ..Rect::auto()
            };
            tree.set_style(child, style).unwrap();
            layout(&mut tree, root);
            let after = tree.computed_layout_output(root).unwrap().unwrap();
            assert_eq!(after.baselines, before.baselines);
            assert_eq!(after.baselines_x, before.baselines_x);
            assert_eq!(
                tree.unrounded_layout(child).location,
                Point {
                    x: origin.x + 7.0,
                    y: origin.y + 11.0
                }
            );
            layout(&mut tree, root);
            assert_eq!(tree.computed_layout_output(root).unwrap(), Some(after));
        }
    }
}

#[test]
fn grid_and_lanes_retain_last_only_x_and_containment() {
    for lanes in [None, Some(lane(false)), Some(lane(true))] {
        for contain in [Contain::NONE, Contain::LAYOUT] {
            let mut tree = TaffyTree::new();
            let child = tree
                .new_leaf_with_context(
                    Style {
                        size: Size::from_lengths(40.0, 40.0),
                        ..Style::DEFAULT
                    },
                    true,
                )
                .unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        display: Display::Grid,
                        grid_lanes: lanes,
                        contain,
                        grid_template_columns: vec![length(40.0)],
                        grid_template_rows: vec![length(40.0)],
                        ..Style::DEFAULT
                    },
                    &[child],
                )
                .unwrap();
            layout(&mut tree, root);
            let result = tree.computed_layout_output(root).unwrap().unwrap();
            assert_eq!(result.baselines_x.first, None);
            assert_eq!(
                result.baselines_x.last,
                (contain == Contain::NONE).then_some(-3.0)
            );
            if lanes == Some(lane(false)) && contain == Contain::NONE {
                assert_eq!(result.baselines.first, None);
                assert_eq!(result.baselines.last, Some(55.0));
            }
        }
    }
}

#[test]
fn scrollable_block_keeps_intentionally_hidden_inline_block_baseline() {
    let mut tree: TaffyTree<()> = TaffyTree::new();
    let content = tree
        .new_leaf(Style {
            size: Size::from_lengths(20.0, 20.0),
            ..Style::DEFAULT
        })
        .unwrap();
    let table = tree
        .new_with_children(
            Style {
                display: Display::Grid,
                no_inline_block_baseline: true,
                overflow: Point {
                    x: Overflow::Hidden,
                    y: Overflow::Hidden,
                },
                ..Style::DEFAULT
            },
            &[content],
        )
        .unwrap();
    let root = tree
        .new_with_children(
            Style {
                display: Display::Block,
                ..Style::DEFAULT
            },
            &[table],
        )
        .unwrap();
    tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
    assert_eq!(
        tree.computed_layout_output(root)
            .unwrap()
            .unwrap()
            .inline_block_last_y(),
        None
    );
}
