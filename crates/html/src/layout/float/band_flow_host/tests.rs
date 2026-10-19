//! Native container shrink-to-fit preserves intrinsic and content-box sizes.

use super::*;

#[test]
fn multicol_float_claims_the_used_content_width_only_after_intrinsic_measurement() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut style = Computed {
        column_count: Some(2),
        ..Default::default()
    };
    style.padding.left = Some(Len::Px(30.));
    style.padding.right = Some(Len::Px(30.));
    let inherited = Computed::default();
    fit_own_layout(&mut style, &inherited, 0., 0., doc.values());
    assert!(style.width.is_none());
    fit_own_layout(&mut style, &inherited, 100., 784., doc.values());
    assert_eq!(style.width, Some(Len::Px(40.)));
}

#[test]
fn specified_multicol_width_is_not_replaced_by_the_probe_width() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut style = Computed {
        column_count: Some(2),
        width: Some(Len::Px(20.)),
        ..Default::default()
    };
    fit_own_layout(&mut style, &Computed::default(), 100., 784., doc.values());
    assert_eq!(style.width, Some(Len::Px(20.)));
}

#[test]
fn invisible_inline_run_uses_the_paragraph_without_becoming_an_atom() {
    let mut span = Element::synthetic("span", Computed::default(), Default::default(), vec![]);
    span.inline = true;
    for children in [vec![], vec![Node::Text("\n  ".into())]] {
        span.children = children;
        assert_eq!(
            inline_run::classify(&[Node::Element(span.clone())]),
            (false, true)
        );
    }
}
