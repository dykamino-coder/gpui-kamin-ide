//! KaminIDE patch: normal-flow baseline сохраняется при перестановке физических осей блока.
use crate::prelude::*;
use crate::{BlockFlow, MeasureOutput, Overflow, Point};

#[test]
fn vertical_block_baselines_ignore_relative_offsets_in_both_directions() {
    for reverse in [false, true] {
        for overflow in [Overflow::Visible, Overflow::Hidden] {
            let mut tree: TaffyTree<()> = TaffyTree::new();
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
                    (),
                )
                .unwrap();
            let root = tree
                .new_with_children(
                    Style {
                        display: Display::Block,
                        block_flow: Some(BlockFlow {
                            vertical: true,
                            block_reverse: reverse,
                            inline_reverse: false,
                        }),
                        size: Size::from_lengths(100.0, 100.0),
                        padding: Rect::length(5.0),
                        border: Rect::length(2.0),
                        ..Style::DEFAULT
                    },
                    &[child],
                )
                .unwrap();
            let compute = |tree: &mut TaffyTree<()>| {
                tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
                    crate::compute_leaf_layout(
                        input,
                        style,
                        |_, _| 0.0,
                        |_, _| MeasureOutput {
                            size: Size { width: 40.0, height: 40.0 },
                            baseline: Some(-3.0),
                            last_baseline: Some(55.0),
                            baseline_x: Some(55.0),
                            last_baseline_x: Some(-3.0),
                            baseline_x_from_right: false,
                        },
                    )
                })
                .unwrap();
            };
            compute(&mut tree);
            let before = tree.computed_layout_output(root).unwrap().unwrap();
            let origin = tree.unrounded_layout(child).location;
            let mut style = tree.style(child).unwrap().clone();
            style.inset = Rect {
                left: length(7.0),
                top: length(11.0),
                ..Rect::auto()
            };
            tree.set_style(child, style).unwrap();
            compute(&mut tree);
            let after = tree.computed_layout_output(root).unwrap().unwrap();
            assert_eq!(after.baselines, before.baselines);
            assert_eq!(after.baselines_x, before.baselines_x);
            assert_eq!(
                tree.unrounded_layout(child).location,
                Point {
                    x: origin.x + 7.0,
                    y: origin.y + 11.0,
                }
            );
            assert!(before.baselines.first.is_some() && before.baselines.last.is_some());
            assert!(before.baselines_x.first.is_some() && before.baselines_x.last.is_some());
        }
    }
}
