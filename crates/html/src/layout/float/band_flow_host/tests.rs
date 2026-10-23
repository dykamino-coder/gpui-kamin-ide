//! Тесты: нативный shrink-to-fit контейнера сохраняет intrinsic-размеры и размеры content-box.

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

#[test]
fn empty_inline_fragment_keeps_its_margin_in_the_paragraph() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut span = Element::synthetic("span", Computed::default(), Default::default(), vec![]);
    span.inline = true;
    span.style.cascaded_mut().margin.left = Some(Len::Px(40.));
    let nodes = vec![Node::Element(span)];
    let lifted = band_flow_rest_lift(nodes, 16., None, WritingMode::default(), None, doc.values())
        .expect("empty inline fragment belongs to the paragraph");
    let Node::Element(run) = &lifted[0] else {
        panic!("anonymous paragraph");
    };
    assert!(run.marks.anon && !run.marks.atoms);
    let Node::Element(span) = &run.children[0] else {
        panic!("inline fragment");
    };
    assert_eq!(span.style.cascaded().margin.left, Some(Len::Px(40.)));
    assert!(span.children.is_empty());
}

#[test]
fn unknown_atomic_container_sizes_belong_to_the_paragraph() {
    for display in [
        Display::InlineBlock,
        Display::InlineFlex,
        Display::InlineGrid,
        Display::InlineTable,
    ] {
        let container = Element::synthetic(
            "div",
            Computed {
                display: Some(display),
                ..Default::default()
            },
            Default::default(),
            vec![Node::Element(Element::synthetic(
                "div",
                Computed::default(),
                Default::default(),
                vec![],
            ))],
        );
        assert_eq!(
            inline_run::classify(&[Node::Element(container)]),
            (false, true)
        );
    }
}

#[test]
fn nested_lift_stops_at_the_absolute_containing_block() {
    use crate::style::computed::Position;

    let pinned = Element::synthetic(
        "div",
        Computed {
            position: Some(Position::Absolute),
            inset: crate::style::computed::Sides {
                top: Some(Len::Px(10.)),
                left: Some(Len::Px(20.)),
                ..Default::default()
            },
            ..Default::default()
        },
        Default::default(),
        vec![],
    );
    for position in [None, Some(Position::Relative)] {
        let mut ancestor = Element::synthetic(
            "div",
            Computed {
                position,
                ..Default::default()
            },
            Default::default(),
            vec![Node::Element(pinned.clone())],
        );
        let mut lifted = vec![];
        nested_lift::lift_nested(&mut ancestor, 16., WritingMode::default(), &mut lifted);
        assert_eq!(lifted.len(), usize::from(position.is_none()));
        assert_eq!(ancestor.children.len(), usize::from(position.is_some()));
    }
}
