//! Схлопывание горизонтальных полей в вертикальном письме только для обычного блочного
//! потока: элементы flex и сетки (`grid`, `inline-grid`, `flex`, `inline-flex`, `grid-lanes`)
//! сохраняют свои поля по обеим физическим осям, остальные дети идут в
//! `collapse_flow_margins`. Точка входа из `generic_box::children`.
use crate::dom::Node;
use crate::style::computed::{Computed, Display};

pub(crate) fn children(
    children: Vec<Node>,
    parent: &Computed,
    reverse: bool,
    lead: Option<f32>,
    vcx: crate::document::state::ValueCx<'_>,
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
        // `em` margins resolve against the children's font size, which falls
        // back to this container's (CSS 2.1 §15.7), as on the horizontal path
        // (`render::blocks::prepare_children`); the collapse step runs before
        // the children's styles are inherited (`clearance-calculations-vrl-002`:
        // `margin-right: 4em` at 20px collapsed as 64px).
        let base = match parent.font_size {
            Some(crate::style::values::value::Len::Px(v)) => v,
            _ => vcx.collapse.font,
        };
        let vcx = vcx.with_collapse(crate::layout::block::margins::CollapseInputs {
            font: base,
            ..vcx.collapse
        });
        crate::layout::block::margins::collapse_flow_margins(children, reverse, lead, vcx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    fn items() -> Vec<Node> {
        (0..3)
            .map(|_| {
                let mut element =
                    crate::layout::table::anon::anon_element("div", vec![Node::Text("É".into())]);
                element.style.margin.left = Some(Len::Px(12.0));
                element.style.margin.right = Some(Len::Px(6.0));
                Node::Element(element)
            })
            .collect()
    }

    #[test]
    fn vertical_grid_and_flex_keep_asymmetric_item_margins_in_both_directions() {
        let metric_doc = crate::document::state::DocState::new(Default::default());
        let vcx = metric_doc.values();
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
                    writing_mode: Some(crate::style::computed::writing_mode::WritingModeKw::new(
                        true, false, false,
                    )),
                    ..Computed::default()
                };
                for node in children(items(), &parent, reverse, Some(0.0), vcx) {
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
        let metric_doc = crate::document::state::DocState::new(Default::default());
        let vcx = metric_doc.values();
        let parent = Computed {
            writing_mode: Some(crate::style::computed::writing_mode::WritingModeKw::new(
                true, false, false,
            )),
            ..Computed::default()
        };
        let result = children(items(), &parent, false, None, vcx);
        let Node::Element(second) = &result[1] else {
            panic!("element expected")
        };
        assert_eq!(second.style.margin.left, Some(Len::Px(6.0)));
        assert_eq!(second.style.margin.right, Some(Len::Px(6.0)));
    }

    #[test]
    fn inline_boxes_keep_their_margins_and_separate_block_neighbors() {
        let metric_doc = crate::document::state::DocState::new(Default::default());
        let vcx = metric_doc.values();
        for reverse in [false, true] {
            let mut input = items();
            let Node::Element(middle) = &mut input[1] else {
                unreachable!()
            };
            middle.style.display = Some(Display::InlineBlock);
            let output = children(input, &Computed::default(), reverse, None, vcx);
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
        let metric_doc = crate::document::state::DocState::new(Default::default());
        let vcx = metric_doc.values();
        for (text, expected_left) in [("text", 12.0), (" \n ", 6.0)] {
            let mut input = items();
            input.insert(1, Node::Text(text.into()));
            let output = children(input, &Computed::default(), false, None, vcx);
            let Node::Element(second) = &output[2] else {
                unreachable!()
            };
            assert_eq!(second.style.margin.left, Some(Len::Px(expected_left)));
        }
    }
}
