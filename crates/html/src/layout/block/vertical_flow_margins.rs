//! Grid and flex items preserve their own margins on both physical axes.
//! Horizontal block-margin collapse applies only to ordinary vertical block flow.
use crate::{
    computed::{Computed, Display},
    dom::Node,
};

pub(crate) fn children(
    children: Vec<Node>,
    parent: &Computed,
    reverse: bool,
    lead: Option<f32>,
) -> Vec<Node> {
    if matches!(
        parent.display,
        Some(
            Display::Grid
                | Display::InlineGrid
                | Display::Flex
                | Display::InlineFlex
                | Display::GridLanes
        )
    ) {
        children
    } else {
        crate::render::collapse_flow_margins(children, reverse, lead)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Len;

    fn items() -> Vec<Node> {
        (0..3)
            .map(|_| {
                let mut element = crate::render::anon_element("div", vec![Node::Text("É".into())]);
                element.style.margin.left = Some(Len::Px(12.0));
                element.style.margin.right = Some(Len::Px(6.0));
                Node::Element(element)
            })
            .collect()
    }

    #[test]
    fn vertical_grid_and_flex_keep_asymmetric_item_margins_in_both_directions() {
        for display in [
            Display::Grid,
            Display::InlineGrid,
            Display::Flex,
            Display::InlineFlex,
            Display::GridLanes,
        ] {
            for reverse in [false, true] {
                let parent = Computed {
                    display: Some(display),
                    vertical: Some(true),
                    ..Computed::default()
                };
                for node in children(items(), &parent, reverse, Some(0.0)) {
                    let Node::Element(item) = node else {
                        panic!("element expected")
                    };
                    assert_eq!(item.style.margin.left, Some(Len::Px(12.0)));
                    assert_eq!(item.style.margin.right, Some(Len::Px(6.0)));
                }
            }
        }
    }

    #[test]
    fn ordinary_vertical_blocks_still_collapse_adjacent_positive_margins() {
        let parent = Computed {
            vertical: Some(true),
            ..Computed::default()
        };
        let result = children(items(), &parent, false, None);
        let Node::Element(second) = &result[1] else {
            panic!("element expected")
        };
        assert_eq!(second.style.margin.left, Some(Len::Px(6.0)));
        assert_eq!(second.style.margin.right, Some(Len::Px(6.0)));
    }

    #[test]
    fn inline_boxes_keep_their_margins_and_separate_block_neighbors() {
        for reverse in [false, true] {
            let mut input = items();
            let Node::Element(middle) = &mut input[1] else {
                unreachable!()
            };
            middle.style.display = Some(Display::InlineBlock);
            let output = children(input, &Computed::default(), reverse, None);
            for index in [1, 2] {
                let Node::Element(item) = &output[index] else {
                    unreachable!()
                };
                assert_eq!(item.style.margin.left, Some(Len::Px(12.0)));
                assert_eq!(item.style.margin.right, Some(Len::Px(6.0)));
            }
        }
    }

    #[test]
    fn text_lines_separate_block_margins_but_whitespace_does_not() {
        for (text, expected_left) in [("text", 12.0), (" \n ", 6.0)] {
            let mut input = items();
            input.insert(1, Node::Text(text.into()));
            let output = children(input, &Computed::default(), false, None);
            let Node::Element(second) = &output[2] else {
                unreachable!()
            };
            assert_eq!(second.style.margin.left, Some(Len::Px(expected_left)));
        }
    }
}
