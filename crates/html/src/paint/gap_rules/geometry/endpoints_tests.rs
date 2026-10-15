//! Crossing size is independent of the inset property selected at an endpoint.

use super::{Crossing, GapRun};

#[test]
fn interior_caps_keep_the_geometric_crossing_width() {
    let mut run = GapRun {
        g0: 10.,
        g1: 30.,
        r0: 0.,
        r1: 220.,
        crossings: vec![Crossing {
            lo: 100.,
            hi: 120.,
            breaks: true,
            joins: false,
            cross_w: 3.,
        }],
        blocked: vec![],
        hidden: vec![],
        start_edge: None,
        end_edge: None,
        index: 0,
        count: 1,
    };
    assert_eq!(run.edge(100., false), (100., 20., false, 3.));
    assert_eq!(run.edge(120., true), (120., 20., false, 3.));
    assert_eq!(run.edge(0., true), (0., 0., false, 0.));
    assert_eq!(run.edge(220., false), (220., 0., false, 0.));
    run.crossings[0].joins = true;
    assert_eq!(run.edge(100., false), (100., 20., true, 3.));
}
