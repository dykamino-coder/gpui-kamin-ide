//! Grid layout supplies margin-box space to measured leaves and nested flex items.
#[cfg(feature = "flexbox")]
mod tests {
    use crate::prelude::*;
    use crate::{AlignItems, compute_leaf_layout};

    fn item_width(margin: f32, nested: bool) -> f32 {
        item_width_with_keyword(margin, nested, false)
    }

    fn item_width_with_keyword(margin: f32, nested: bool, keyword: bool) -> f32 {
        let mut tree: TaffyTree<()> = TaffyTree::new();
        tree.disable_rounding();
        let text = tree.new_leaf_with_context(Style::DEFAULT, ()).unwrap();
        let item = if nested {
            tree.new_with_children(
                Style {
                    display: Display::Flex,
                    size: Size {
                        width: if keyword {
                            Dimension::fit_content()
                        } else {
                            auto()
                        },
                        height: auto(),
                    },
                    margin: Rect {
                        top: length(margin),
                        right: length(0.0),
                        bottom: length(0.0),
                        left: length(0.0),
                    },
                    ..Style::DEFAULT
                },
                &[text],
            )
            .unwrap()
        } else {
            tree.set_style(
                text,
                Style {
                    size: Size {
                        width: if keyword {
                            Dimension::fit_content()
                        } else {
                            auto()
                        },
                        height: auto(),
                    },
                    margin: Rect {
                        top: length(margin),
                        right: length(0.0),
                        bottom: length(0.0),
                        left: length(0.0),
                    },
                    ..Style::DEFAULT
                },
            )
            .unwrap();
            text
        };
        let root = tree
            .new_with_children(
                Style {
                    display: Display::Grid,
                    size: Size {
                        width: length(200.0),
                        height: length(100.0),
                    },
                    grid_template_columns: vec![length(200.0)],
                    grid_template_rows: vec![length(100.0)],
                    justify_items: Some(AlignItems::START),
                    align_items: Some(AlignItems::START),
                    ..Style::DEFAULT
                },
                &[item],
            )
            .unwrap();
        tree.compute_layout_with_measure(root, Size::MAX_CONTENT, |input, _, _, style| {
            compute_leaf_layout(
                input,
                style,
                |_, _| 0.0,
                |known, available| {
                    let height = known.height.unwrap_or(match available.height {
                        AvailableSpace::Definite(value) => value.min(75.0),
                        AvailableSpace::MinContent => 40.0,
                        AvailableSpace::MaxContent => 75.0,
                    });
                    let width = if height < 75.0 { 40.0 } else { 20.0 };
                    Size {
                        width: known.width.unwrap_or(width),
                        height,
                    }
                },
            )
        })
        .unwrap();
        let layout = tree.layout(item).unwrap();
        layout.size.width
    }

    #[test]
    fn fit_content_width_preserves_margin_box_cross_space() {
        for nested in [false, true] {
            for (margin, expected) in [(0.0, 20.0), (15.0, 20.0), (20.0, 20.0), (30.0, 40.0)] {
                assert_eq!(item_width_with_keyword(margin, nested, true), expected);
            }
        }
    }

    #[test]
    fn a_grid_item_margin_is_deducted_once_before_vertical_text_wrapping() {
        // Both 85px and 80px of content space fit the same 75px text line.
        // A 30px margin leaves 70px and legitimately requires two columns.
        for nested in [false, true] {
            assert_eq!(item_width(0.0, nested), 20.0);
            assert_eq!(item_width(15.0, nested), 20.0);
            assert_eq!(item_width(20.0, nested), 20.0);
            assert_eq!(item_width(30.0, nested), 40.0);
        }
    }
}
