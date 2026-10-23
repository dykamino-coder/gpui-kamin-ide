//! Тесты: нулевая used-высота не становится определённым ортогональным containing block.

use super::*;
use crate::dom::Element;
use crate::layout::block::margin_height::used_style;
use crate::layout::writing_mode::orthogonal_inline;
use crate::style::computed::writing_mode::WritingModeKw;
use crate::style::computed::{BoxStyle, Computed};

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
        assert_eq!(block.style.computed().height, height);
        assert!(block.marks.auto_zero_height);
        let painted = used_style(block, &BoxStyle::from((*block.style.cascaded()).clone()))
            .unwrap_or_else(|| BoxStyle::from(block.style.cascaded().clone()));
        assert_eq!(painted.height, Some(Len::Px(0.0)));
        let mut child = {
            let mut with_inherited = Computed {
                floated_box: true,
                ..Default::default()
            };
            with_inherited.writing_mode = Some(WritingModeKw::new(true, false, false));
            with_inherited
        };
        orthogonal_inline::resolve(&mut child, &block.style.computed(), (800.0, 600.0), false);
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
