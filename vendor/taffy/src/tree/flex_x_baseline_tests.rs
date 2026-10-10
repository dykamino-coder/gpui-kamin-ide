//! Intrinsic block items must preserve physical x baselines in vertical alignment.
use crate::prelude::{length, TaffyAuto, TaffyMaxContent};
use crate::{AlignItems, Baselines, Dimension, Display, Size, Style, TaffyTree};

#[test]
fn column_flex_share_first_and_last_vertical_baselines() {
    for direction in [
        crate::FlexDirection::Column,
        crate::FlexDirection::ColumnReverse,
    ] {
        for flags in [4, 5] {
            for width in [Dimension::min_content(), Dimension::max_content()] {
                for last in [false, true] {
                    let mut tree: TaffyTree<(f32, f32, f32)> = TaffyTree::new();
                    tree.disable_rounding();
                    let contexts = [(20.0, 6.0, 14.0), (30.0, 9.0, 24.0)];
                    let mut items = vec![];
                    for context in contexts {
                        let text = tree.new_leaf_with_context(Style::DEFAULT, context).unwrap();
                        items.push(
                            tree.new_with_children(
                                Style {
                                    display: Display::Block,
                                    baseline_x_flags: flags,
                                    size: Size {
                                        width,
                                        height: Dimension::AUTO,
                                    },
                                    ..Style::DEFAULT
                                },
                                &[text],
                            )
                            .unwrap(),
                        );
                    }
                    let root = tree
                        .new_with_children(
                            Style {
                                display: Display::Flex,
                                flex_direction: direction,
                                size: Size {
                                    width: length(80.0),
                                    height: Dimension::AUTO,
                                },

                                align_items: if last {
                                    AlignItems::LAST_BASELINE
                                } else {
                                    AlignItems::BASELINE
                                },
                                ..Style::DEFAULT
                            },
                            &items,
                        )
                        .unwrap();
                    tree.compute_layout_with_measure(
                        root,
                        Size::MAX_CONTENT,
                        |inputs, _, context, style| {
                            let (width, first, last) = *context.unwrap();
                            let mut output = crate::compute_leaf_layout(
                                inputs,
                                style,
                                |_, _| 0.0,
                                |known, _| Size {
                                    width: known.width.unwrap_or(width),
                                    height: known.height.unwrap_or(30.0),
                                },
                            );
                            output.baselines_x = Baselines {
                                first: Some(first),
                                last: Some(last),
                            };
                            output
                        },
                    )
                    .unwrap();
                    let mut baselines = vec![];
                    for (&item, &(width, first, final_line)) in items.iter().zip(&contexts) {
                        let layout = tree.layout(item).unwrap();
                        assert_eq!(layout.size.width, width);
                        assert!(layout.location.x >= 0.0 && layout.location.x + width <= 80.0);
                        baselines.push(layout.location.x + if last { final_line } else { first });
                    }
                    assert_eq!(baselines[0], baselines[1], "flags={flags} last={last}");
                }
            }
        }
    }
}
