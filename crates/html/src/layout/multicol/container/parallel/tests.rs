//! Fragment growth preserves the original CB position and its normal-flow size.

use super::*;
use crate::style::computed::Computed;

fn element(style: Computed, children: Vec<Node>) -> Element {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(10_000);
    Element {
        marks: Default::default(),
        node_id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        tag: "div".into(),
        style: style.into(),
        children,
        attrs: Vec::new(),
        inline: false,
        list_item: None,
        anim: None,
        hover: None,
        first_letter: None,
        first_line: None,
    }
}

fn containing_block() -> Element {
    let children = [(90.0, false), (50.0, true), (50.0, false)]
        .into_iter()
        .map(|(h, avoid)| {
            Node::Element(element(
                Computed {
                    height: Some(Len::Px(h)),
                    break_inside_avoid: avoid,
                    ..Default::default()
                },
                Vec::new(),
            ))
        })
        .collect();
    let mut abs = element(
        Computed {
            position: Some(Position::Absolute),
            ..Default::default()
        },
        children,
    );
    abs.style.inset.bottom = Some(Len::Px(0.0));
    element(
        Computed {
            height: Some(Len::Px(190.0)),
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(abs)],
    )
}

#[test]
fn bottom_position_is_resolved_before_auto_height_grows() {
    let mut cb = containing_block();
    assert!(walk(&mut cb, 0.0, 100.0, 2, 4));
    let Node::Element(abs) = &cb.children[0] else {
        panic!()
    };
    assert_eq!(abs.style.inset.top, Some(Len::Px(0.0)));
    assert_eq!(measure(abs).unwrap().0.0, 200.0);
    let (shape, own) = measure(&cb).unwrap();
    assert_eq!(shape.0, 200.0);
    assert_eq!(own, 190.0);
}

#[test]
fn independent_breaks_use_the_containing_blocks_fragmentainer_phase() {
    let mut cb = containing_block();
    assert!(walk(&mut cb, 80.0, 100.0, 2, 4));
    let Node::Element(abs) = &cb.children[0] else {
        panic!()
    };
    assert_eq!(abs.style.inset.top, Some(Len::Px(0.0)));
    assert_eq!(measure(abs).unwrap().0.0, 220.0);
    assert_eq!(measure(&cb).unwrap().1, 190.0);
}
