//! Допуск семейств: normal-узлы не допускаются, OOF — своим токеном; внешний корень
//! рисует вложенные потоки в своей записи; сегмент без копии предков — тождественный допуск.

use super::*;
use gpui::{point, px, size};

fn rect(y: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(0.), px(y)), size(px(100.), px(20.)))
}

fn placement(family: u64, root: u64, segment: u64, dx: f32, copied: bool) -> Placement {
    Placement {
        family,
        root,
        segment: PaintSegmentId(segment),
        source: point(px(0.), px(20.)),
        destination: point(px(dx), px(20.)),
        clip: rect(20.),
        decoration: rect(20.),
        copied,
        transform: None,
        reference: rect(20.),
        start: point(px(0.), px(0.)),
        follows: false,
        origin: point(px(0.), px(20.)),
    }
}

#[test]
fn normal_nodes_are_not_admitted_and_nested_flows_paint_in_the_root_capture() {
    let mut snapshot = Snapshot::default();
    snapshot
        .families
        .extend([(10, 10), (11, 10), (20, 20), (21, 20), (30, 30)]);
    snapshot
        .normal
        .extend([(10, rect(0.)), (20, rect(20.)), (30, rect(20.))]);
    let mut admission = Admission::new(false);
    admission.insert(placement(20, 20, 0, 0., true)).unwrap();
    admission.insert(placement(20, 20, 1, 100., false)).unwrap();
    // A flow nested in 20 continues where 20 has no own fragment.
    admission.insert(placement(30, 20, 2, 200., false)).unwrap();
    let routing = admission.finish(&snapshot);
    let (hit_routes, hits) = &routing.hits;
    assert_eq!(hit_routes.len(), 3);
    assert!(hits.nodes[&10].is_empty() && hits.nodes[&11].is_empty());
    assert_eq!(hits.nodes[&21].len(), 2);
    let (paint_routes, paint) = &routing.paint;
    // Segments without a legacy copy re-admit the placed root record unmoved.
    assert_eq!(paint_routes.len(), 2);
    assert!(paint_routes.iter().all(|r| r.source == r.destination));
    assert_eq!(paint.nodes[&20].len(), 2);
    assert!(!paint.nodes.contains_key(&10));
    assert!(!paint.nested.contains_key(&30));
    let own = &paint.nested[&20];
    assert_eq!(&*own.bare, &[PaintSegmentId(0)]);
    assert_eq!(own.routes.len(), 3);
    assert_eq!(own.families[&30].len(), 1);
}

#[test]
fn ordinals_are_per_segment_across_families() {
    let mut admission = Admission::new(false);
    admission.insert(placement(1, 1, 3, 0., true)).unwrap();
    admission.insert(placement(2, 2, 3, 0., true)).unwrap();
    let routing = admission.finish(&Snapshot::default());
    let ordinals: Vec<_> = routing.hits.0.iter().map(|r| (r.id, r.ordinal)).collect();
    assert_eq!(ordinals, [(PaintSegmentId(3), 0), (PaintSegmentId(3), 1)]);
}
