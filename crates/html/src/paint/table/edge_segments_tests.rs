//! Tests of collapsed edge candidates and joints.
use super::{Cand, Seg, draw_cands, key_order};
use crate::style::values::value::Color;

#[test]
fn nan_widths_and_lines_keep_a_total_order() {
    // 64 candidates, every third with a NaN line or width: the sorts and
    // the winner choice must not panic (`sort_by` checks total order).
    let mut cands: Vec<Cand> = (0..64u32)
        .map(|i| {
            let nan = i % 3 == 0;
            Cand {
                line: if nan {
                    f32::NAN
                } else {
                    ((i * 37) % 101) as f32
                },
                a: 0.0,
                b: 10.0,
                w: if nan { f32::NAN } else { 1.0 + (i % 4) as f32 },
                style: 9,
                source: 0,
                doc_ix: i,
                colour: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
                outward: 0,
            }
        })
        .collect();
    let mut segs: Vec<Seg> = vec![];
    draw_cands(
        &mut cands,
        true,
        None,
        None,
        &|_, _| 0.5,
        &|_, _| 0.5,
        &mut segs,
    );
    assert!(!segs.is_empty());
    segs.sort_by(|p, q| key_order(&p.0, &q.0));
    assert!(segs.windows(2).all(|w| key_order(&w[0].0, &w[1].0).is_le()));
}
/// Blink `ComputeEdgeJoints`: on the block-start fragmentation boundary
/// the joint has no block size, so the vertical below keeps the corner
/// right of the half horizontal edge (`table-collapsed-borders-paint-at-
/// boundary.tentative`: black vertical over the lime half edge).
#[test]
fn half_edge_on_the_fragment_top_leaves_its_end_corner_to_the_vertical() {
    let colour = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    let cand = || Cand {
        line: 100.0,
        a: 0.0,
        b: 90.0,
        w: 10.0,
        style: 9,
        source: 2,
        doc_ix: 1,
        colour,
        outward: 0,
    };
    let end = |cut: Option<f32>| {
        let mut segs: Vec<Seg> = vec![];
        draw_cands(
            &mut [cand()],
            false,
            cut,
            Some(0.0),
            &|_, _| 5.0,
            &|_, _| 5.0,
            &mut segs,
        );
        let r = segs[0].2;
        f32::from(r.origin.x + r.size.width)
    };
    assert_eq!(end(None), 95.0);
    assert_eq!(end(Some(100.0)), 85.0);
    // A horizontal elsewhere in the fragment keeps the usual joint.
    assert_eq!(end(Some(40.0)), 95.0);
}
