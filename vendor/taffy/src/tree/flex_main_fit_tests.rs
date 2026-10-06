//! Main fit formulas retain intrinsic floors, box edges, and flex-basis precedence.
    use crate::prelude::*;

    #[test]
    fn main_fit_argument_is_not_replaced_by_the_widest_wrapped_line() {
        for dir in [FlexDirection::Row, FlexDirection::Column] {
            for (keyword, basis, sizing, expected) in [
                (Dimension::fit_content_px(394.0), Dimension::AUTO, BoxSizing::ContentBox, 400.0),
                (Dimension::fit_content_px(400.0), Dimension::AUTO, BoxSizing::BorderBox, 400.0),
                (Dimension::fit_content_px(4.0), Dimension::AUTO, BoxSizing::ContentBox, 30.0),
                (Dimension::fit_content_px(3000.0), Dimension::AUTO, BoxSizing::ContentBox, 2406.0),
                (Dimension::fit_content_percent(0.5), Dimension::AUTO, BoxSizing::ContentBox, 206.0),
                (Dimension::fit_content(), Dimension::AUTO, BoxSizing::ContentBox, 400.0),
                (Dimension::fit_content_px(394.0), Dimension::content(), BoxSizing::ContentBox, 2406.0),
                (Dimension::length(100.0), Dimension::fit_content_px(394.0), BoxSizing::ContentBox, 400.0),
            ] {
                let mut tree: TaffyTree<()> = TaffyTree::new();
                tree.disable_rounding();
                let text = tree.new_leaf(Style::DEFAULT).unwrap();
                let block = tree.new_with_children(Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    flex_basis: basis,
                    flex_shrink: 0.0,
                    size: if dir == FlexDirection::Row { Size { width: keyword, height: Dimension::AUTO } } else { Size { width: Dimension::AUTO, height: keyword } },
                    border: Rect { left: length(3.0), right: length(3.0), top: length(3.0), bottom: length(3.0) },
                    box_sizing: sizing,
                    ..Style::DEFAULT
                }, &[text]).unwrap();
                let outer = tree.new_with_children(Style {
                    display: Display::Flex,
                    flex_direction: dir,
                    size: if dir == FlexDirection::Row { Size { width: length(400.0), height: Dimension::AUTO } } else { Size { width: Dimension::AUTO, height: length(400.0) } },
                    ..Style::DEFAULT
                }, &[block]).unwrap();
                tree.compute_layout_with_measure(outer, Size::MAX_CONTENT, |inputs, _, _, style| {
                    crate::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, available| {
                        let axis = |space| match space {
                            AvailableSpace::MinContent => 24.0,
                            AvailableSpace::MaxContent => 2400.0,
                            AvailableSpace::Definite(cap) => (cap / 24.0).floor().max(1.0) * 24.0,
                        };
                        if dir == FlexDirection::Row {
                            Size { width: known.width.unwrap_or_else(|| axis(available.width)), height: 100.0 }
                        } else {
                            Size { width: 100.0, height: known.height.unwrap_or_else(|| axis(available.height)) }
                        }
                    })
                }).unwrap();
                assert_eq!(if dir == FlexDirection::Row { tree.layout(block).unwrap().size.width } else { tree.layout(block).unwrap().size.height }, expected, "{dir:?} {keyword:?} {basis:?} {sizing:?}");
            }
        }
    }

    #[test]
    fn nested_grid_wrapper_keeps_its_intrinsic_border_box() {
        for parent_width in [300.0, 784.0, 900.0] {
            for edges in [0.0, 7.0] {
                for keyword in [Dimension::AUTO, Dimension::fit_content(), Dimension::min_content(), Dimension::max_content()] {
                    let mut tree: TaffyTree<()> = TaffyTree::new();
                    tree.disable_rounding();
                    let leaf = tree.new_leaf(Style {
                        size: Size { width: length(100.0), height: length(100.0) },
                        ..Style::DEFAULT
                    }).unwrap();
                    let actual = tree.new_with_children(Style {
                        display: Display::Grid,
                        padding: Rect { left: length(edges), right: length(edges), top: length(0.0), bottom: length(0.0) },
                        ..Style::DEFAULT
                    }, &[leaf]).unwrap();
                    let wrapper = tree.new_with_children(Style {
                        display: Display::Grid,
                        size: Size { width: keyword, height: Dimension::AUTO },
                        justify_items: Some(AlignItems::START),
                        ..Style::DEFAULT
                    }, &[actual]).unwrap();
                    let outer = tree.new_with_children(Style {
                        display: Display::Flex,
                        flex_direction: FlexDirection::Column,
                        size: Size { width: length(parent_width), height: Dimension::AUTO },
                        ..Style::DEFAULT
                    }, &[wrapper]).unwrap();
                    tree.compute_layout(outer, Size::MAX_CONTENT).unwrap();
                    let expected = if keyword.is_auto() { parent_width } else { 100.0 + 2.0 * edges };
                    assert_eq!(tree.layout(wrapper).unwrap().size.width, expected);
                    assert_eq!(tree.layout(actual).unwrap().size.width, 100.0 + 2.0 * edges);
                    assert_eq!(tree.layout(leaf).unwrap().size.width, 100.0);
                }
            }
        }
    }
