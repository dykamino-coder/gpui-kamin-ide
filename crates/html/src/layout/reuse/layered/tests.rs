use super::*;
use crate::dom::Marks;

fn el(position: Option<Position>, transform: bool, children: Vec<Element>) -> Element {
    let mut style = Computed::default();
    style.reset_positioned.position = position;
    if transform {
        style.reset_transforms.transform = Some(Default::default());
    }
    let children = children.into_iter().map(Node::Element).collect();
    Element::synthetic("div", style, Marks::default(), children)
}

#[test]
fn out_of_flow_boxes_must_have_their_containing_block_inside() {
    let abs = || el(Some(Position::Absolute), false, vec![]);
    let fixed = || el(Some(Position::Fixed), false, vec![]);
    assert!(contained(&el(None, false, vec![el(None, false, vec![])])));
    assert!(contained(&el(Some(Position::Relative), false, vec![abs()])));
    assert!(
        !contained(&el(None, false, vec![abs()])),
        "abspos placed outside"
    );
    assert!(
        contained(&el(
            None,
            false,
            vec![el(Some(Position::Relative), false, vec![abs()])]
        )),
        "a positioned ancestor inside"
    );
    assert!(
        !contained(&el(Some(Position::Relative), false, vec![fixed()])),
        "fixed placed by the viewport"
    );
    assert!(
        contained(&el(None, true, vec![fixed()])),
        "a transform contains fixed boxes"
    );
    assert!(
        !contained(&el(Some(Position::Absolute), false, vec![])),
        "the block is out of flow"
    );
    assert!(!contained(&el(
        Some(Position::Relative),
        false,
        vec![el(Some(Position::Sticky), false, vec![])]
    )));
}
