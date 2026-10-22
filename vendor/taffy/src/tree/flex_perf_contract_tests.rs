//! Flex-пробы сохраняют финальные метрики, кэш и процентную базу.
use crate::prelude::*;
use crate::{Baselines, MeasureOutput, RunMode};

fn compute(tree: &mut TaffyTree<Vec<f32>>, root: NodeId, absent: bool, finals: &mut usize) {
    tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, context, style| {
        let final_pass = input.run_mode == RunMode::PerformLayout;
        if final_pass {
            *finals += 1;
            *context.unwrap() = if absent { vec![] } else { vec![7.0, 17.0] };
        }
        let offset = if final_pass { 0.0 } else { -900.0 };
        crate::compute_leaf_layout(
            input,
            style,
            |_, _| 0.0,
            |_, _| MeasureOutput {
                size: Size {
                    width: 40.0,
                    height: 20.0,
                },
                baseline: (!absent).then_some(7.0 + offset),
                last_baseline: (!absent).then_some(17.0 + offset),
                baseline_x: (!absent).then_some(11.0 + offset),
                last_baseline_x: (!absent).then_some(23.0 + offset),
                baseline_x_from_right: true,
            },
        )
    })
    .unwrap();
}

#[test]
fn stretch_probe_elision_keeps_completed_metadata_and_dirty_lifecycle() {
    for direction in [
        FlexDirection::Row,
        FlexDirection::RowReverse,
        FlexDirection::Column,
        FlexDirection::ColumnReverse,
    ] {
        let is_row = matches!(direction, FlexDirection::Row | FlexDirection::RowReverse);
        for wrap in [FlexWrap::NoWrap, FlexWrap::Wrap] {
            for alignment in [
                AlignItems::STRETCH,
                AlignItems::BASELINE,
                AlignItems::LAST_BASELINE,
            ] {
                for mode in 0..3 {
                    for absent in [false, true] {
                        for intrinsic in [false, true] {
                            let mut tree: TaffyTree<Vec<f32>> = TaffyTree::new();
                            tree.disable_rounding();
                            let mut style = Style {
                                flex_basis: length(20.0),
                                flex_grow: 1.0,
                                min_size: Size::zero(),
                                padding: Rect::length(3.0),
                                border: Rect::length(2.0),
                                ..Style::DEFAULT
                            };
                            if mode == 1 {
                                if is_row {
                                    style.margin.top = auto();
                                } else {
                                    style.margin.left = auto();
                                }
                            } else if mode == 2 {
                                if is_row {
                                    style.size.height = Dimension::stretch();
                                } else {
                                    style.size.width = Dimension::stretch();
                                }
                            }
                            let child = tree.new_leaf_with_context(style, Vec::new()).unwrap();
                            let root = tree
                                .new_with_children(
                                    Style {
                                        flex_direction: direction,
                                        flex_wrap: wrap,
                                        align_items: alignment,
                                        size: Size {
                                            width: if intrinsic && !is_row {
                                                Dimension::max_content()
                                            } else {
                                                length(200.0)
                                            },
                                            height: if intrinsic && is_row {
                                                Dimension::max_content()
                                            } else {
                                                length(100.0)
                                            },
                                        },
                                        ..Style::DEFAULT
                                    },
                                    &[child],
                                )
                                .unwrap();
                            let mut finals = 0;
                            compute(&mut tree, root, absent, &mut finals);
                            if intrinsic && mode == 0 {
                                let size = tree.layout(root).unwrap().size;
                                assert_eq!(
                                    if is_row { size.height } else { size.width },
                                    if is_row { 30.0 } else { 50.0 }
                                );
                            }
                            let output = tree.computed_layout_output(child).unwrap().unwrap();
                            assert_eq!(
                                output.baselines,
                                if absent {
                                    Baselines::NONE
                                } else {
                                    Baselines {
                                        first: Some(12.0),
                                        last: Some(22.0),
                                    }
                                }
                            );
                            assert_eq!(
                                output.baselines_x,
                                if absent {
                                    Baselines::NONE
                                } else {
                                    Baselines {
                                        first: Some(output.size.width - 16.0),
                                        last: Some(output.size.width - 28.0),
                                    }
                                }
                            );
                            let measured_finals = finals;
                            compute(&mut tree, root, absent, &mut finals);
                            assert_eq!(finals, measured_finals);
                            assert_eq!(tree.computed_layout_output(child).unwrap(), Some(output));
                            // Кэш не вызывает callback и сохраняет измеренные строки.
                            assert_eq!(
                                tree.get_node_context(child).unwrap().as_slice(),
                                if absent { &[][..] } else { &[7.0, 17.0][..] }
                            );
                            let before = finals;
                            assert!(before > 0);
                            tree.compute_layout_with_measure(
                                root,
                                Size::MAX_CONTENT,
                                |_, _, _, _| panic!("completed cache must not measure"),
                            )
                            .unwrap();
                            tree.mark_dirty(child).unwrap();
                            assert_eq!(tree.computed_layout_output(child).unwrap(), None);
                            tree.compute_layout_with_measure(
                                root,
                                Size::MAX_CONTENT,
                                |input, _, _, style| {
                                    if input.run_mode == RunMode::PerformLayout {
                                        finals += 1;
                                    }
                                    crate::compute_leaf_layout(
                                        input,
                                        style,
                                        |_, _| 0.0,
                                        |_, _| Size {
                                            width: 40.0,
                                            height: 20.0,
                                        },
                                    )
                                },
                            )
                            .unwrap();
                            assert!(finals > before);
                            let output = tree.computed_layout_output(child).unwrap().unwrap();
                            assert_eq!(output.baselines, Baselines::NONE);
                            assert_eq!(output.baselines_x, Baselines::NONE);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn intrinsic_main_size_updates_percentage_padding_in_cross_maximum() {
    for intrinsic in [false, true] {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let child = tree
            .new_leaf(Style {
                box_sizing: BoxSizing::ContentBox,
                size: Size {
                    width: length(100.0),
                    height: auto(),
                },
                max_size: Size {
                    width: auto(),
                    height: length(10.0),
                },
                padding: Rect {
                    top: percent(0.1),
                    bottom: percent(0.1),
                    ..Rect::zero()
                },
                ..Style::DEFAULT
            })
            .unwrap();
        let root = tree
            .new_with_children(
                Style {
                    size: Size {
                        width: if intrinsic {
                            Dimension::max_content()
                        } else {
                            length(100.0)
                        },
                        height: length(100.0),
                    },
                    ..Style::DEFAULT
                },
                &[child],
            )
            .unwrap();
        tree.compute_layout(root, Size::MAX_CONTENT).unwrap();
        assert_eq!(tree.layout(root).unwrap().size.width, 100.0);
        assert_eq!(
            tree.layout(child).unwrap().size.height,
            30.0,
            "intrinsic={intrinsic}"
        );
    }
}
