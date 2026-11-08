//! Порядок слоёв через глубокие позиционированные поддеревья.

use super::*;
use crate::document::state::DocState;
use crate::layout::table::anon::anon_element;
use crate::style::{
    computed::{Computed, Position, ResetPositioned, Sides, StyleGroup},
    values::value::Len,
};

fn absolute() -> Node {
    let mut node = anon_element("absolute", vec![]);
    node.style = Computed {
        reset_positioned: StyleGroup::new(ResetPositioned {
            position: Some(Position::Absolute),
            inset: Sides {
                left: Some(Len::Px(0.)),
                top: Some(Len::Px(0.)),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    }
    .into();
    Node::Element(node)
}

#[test]
fn over_layer_keeps_order_before_a_positioned_descendant_at_any_depth() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let doc = DocState::new(Default::default());
            for depth in [0, 8, 9, 32, 33, 20_000] {
                let mut descendant = anon_element("positioned", vec![]);
                descendant.style = Computed {
                    reset_positioned: StyleGroup::new(ResetPositioned {
                        position: Some(Position::Relative),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
                .into();
                for _ in 0..depth {
                    descendant = anon_element("nested", vec![Node::Element(descendant)]);
                }
                let nodes = vec![
                    absolute(),
                    Node::Element(descendant),
                    Node::Element(anon_element("tail", vec![])),
                ];
                let ordered = by_layer(nodes, false, FrameCx::new(&doc));
                let Node::Element(first) = &ordered[0] else {
                    panic!("element expected")
                };
                assert_eq!(first.tag.as_str(), "absolute", "depth {depth}");
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn over_layer_moves_across_ordinary_flow_and_singletons_stay_in_place() {
    let doc = DocState::new(Default::default());
    let ordered = by_layer(
        vec![absolute(), Node::Element(anon_element("flow", vec![]))],
        false,
        FrameCx::new(&doc),
    );
    let Node::Element(last) = &ordered[1] else {
        panic!("element expected")
    };
    assert_eq!(last.tag.as_str(), "absolute");
    assert_eq!(
        by_layer(vec![absolute()], false, FrameCx::new(&doc)).len(),
        1
    );
}
