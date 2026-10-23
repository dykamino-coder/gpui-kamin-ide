//! Тесты: нулевая used-высота не становится определённым ортогональным containing block.

use super::*;
use crate::dom::Element;
use crate::layout::block::margin_height::used_style;
use crate::layout::writing_mode::orthogonal_inline;
use crate::style::computed::Computed;
use crate::style::computed::writing_mode::WritingModeKw;

#[test]
fn float_only_block_preserves_auto_for_orthogonal_intrinsic_sizing() {
    let doc = crate::document::state::DocState::new(Default::default());
    for height in [None, Some(Len::Auto), Some(Len::Px(0.0))] {
        let float = Element::synthetic(
            "div",
            Computed {
                float: Some(-1),
                ..Default::default()
            },
            Default::default(),
            vec![],
        );
        let block = Element::synthetic(
            "div",
            Computed {
                height,
                ..Default::default()
            },
            Default::default(),
            vec![Node::Element(float)],
        );
        let collapsed = collapse_margins(&[Node::Element(block)], false, None, doc.values());
        let Node::Element(block) = &collapsed[0] else {
            panic!("block")
        };
        assert_eq!(block.style.height, height);
        assert!(block.marks.auto_zero_height);
        let painted = used_style(block, &block.style).unwrap_or_else(|| *block.style.clone());
        assert_eq!(painted.height, Some(Len::Px(0.0)));
        let mut child = Computed {
            writing_mode: Some(WritingModeKw::new(true, false, false)),
            floated_box: true,
            ..Default::default()
        };
        orthogonal_inline::resolve(&mut child, &block.style, (800.0, 600.0), false);
        let available = child.orthogonal_inline.expect("root constraint").available;
        assert_eq!(
            available,
            if height == Some(Len::Px(0.0)) {
                0.0
            } else {
                600.0
            }
        );
    }
}

#[test]
fn fieldset_fragment_keeps_child_end_margin_inside_its_bfc() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut child = Element::synthetic("div", Computed::default(), Default::default(), vec![]);
    child.style.height = Some(Len::Px(70.0));
    child.style.margin.bottom = Some(Len::Px(50.0));
    let fieldset = Element::synthetic(
        "fieldset",
        Computed::default(),
        Default::default(),
        vec![Node::Element(child)],
    );
    let fieldset = fragment_box(fieldset, 50.0, doc.values());
    assert_eq!(fragment_margins(&fieldset), Some((0.0, 0.0)));
    let Node::Element(child) = &fieldset.children[0] else {
        panic!("child")
    };
    assert_eq!(child.style.margin.bottom, Some(Len::Px(50.0)));
}
