//! Парный intrinsic-обход: каждый лист один раз, прежние float/margin правила.

use super::super::{Edge, Nest};
use super::{Kid, Kind, intrinsic_sizes_with};
use std::rc::Rc;

fn kid(id: u64, kind: Kind) -> Kid {
    Kid {
        flow: None,
        watched: None,
        flow_key: 0,
        kind,
        node_id: id,
        pagination_offset: 0.,
        float_floor: None,
        monolithic: false,
        clear: None,
        margin: [Edge::Px(0.); 4],
        build: Rc::new(|_, _, _, _, _| panic!("numeric intrinsic traversal builds no elements")),
        nest: None,
        anon: false,
        lead: None,
        lead_base: None,
        margin_offset: 0.,
        start_open: false,
        shape: None,
        rtl: false,
    }
}

fn float(id: u64, side: i8, clear: Option<i8>) -> Kid {
    kid(
        id,
        Kind::Float {
            side,
            clear,
            shrink: true,
            letter: false,
            own_layout: false,
        },
    )
}

#[test]
fn narrow_boxes_stand_at_the_inline_start_of_their_containing_block() {
    assert_eq!(super::inline_start(10., 400., 230., false), 10.);
    assert_eq!(super::inline_start(10., 400., 230., true), 170.);
}

#[test]
fn nested_intrinsic_sizes_measure_each_leaf_once_for_both_components() {
    let mut root = kid(0, Kind::Flow);
    for id in 1..=12 {
        let mut parent = kid(id, Kind::Nest);
        parent.nest = Some(Nest {
            kids: vec![root],
            inset: [Edge::Px(0.), Edge::Px(5.), Edge::Px(0.), Edge::Px(2.)],
            height: None,
            width: None,
            group: None,
        });
        root = parent;
    }
    let mut calls = 0;
    let sizes = intrinsic_sizes_with(&[root], &mut |leaf| {
        assert_eq!(leaf.node_id, 0);
        calls += 1;
        (11., 37.)
    });
    assert_eq!(sizes, (95., 121.));
    assert_eq!(calls, 1);
}

#[test]
fn opposing_floats_and_signed_fc_margins_keep_both_contributions() {
    let mut left = float(1, -1, None);
    left.margin = [Edge::Px(0.), Edge::Px(5.), Edge::Px(0.), Edge::Px(5.)];
    let right = float(2, 1, None);
    let mut piece = kid(
        3,
        Kind::Piece {
            table: false,
            rtl: false,
        },
    );
    piece.margin = [Edge::Px(0.), Edge::Px(-10.), Edge::Px(0.), Edge::Px(70.)];
    let sizes = intrinsic_sizes_with(&[left, right, piece], &mut |k| match k.node_id {
        1 => (10., 40.),
        2 => (20., 60.),
        _ => (30., 80.),
    });
    assert_eq!(sizes, (90., 200.));
}

#[test]
fn clear_discards_only_the_named_float_side_before_accumulating() {
    let sizes = intrinsic_sizes_with(
        &[
            float(1, -1, None),
            float(2, 1, None),
            float(3, -1, Some(-1)),
        ],
        &mut |k| match k.node_id {
            1 => (10., 50.),
            2 => (20., 60.),
            _ => (75., 75.),
        },
    );
    assert_eq!(sizes, (75., 135.));
}

#[test]
fn anonymous_inline_max_includes_preceding_floats_and_skips_struts() {
    let mut inline = kid(2, Kind::Flow);
    inline.anon = true;
    inline.margin = [Edge::Px(0.), Edge::Px(7.), Edge::Px(0.), Edge::Px(-3.)];
    let mut calls = 0;
    let sizes = intrinsic_sizes_with(
        &[float(1, -1, None), kid(9, Kind::Strut(100.)), inline],
        &mut |k| {
            calls += 1;
            match k.node_id {
                1 => (20., 50.),
                2 => (5., 15.),
                _ => panic!("strut has no intrinsic measurement"),
            }
        },
    );
    assert_eq!(sizes, (20., 69.));
    assert_eq!(calls, 2);
}
