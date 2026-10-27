//! Допуск семейств: normal-узлы не допускаются, OOF — своим токеном; копия предков
//! получает собственную запись корня, сегмент без копии — допуск записи источника.

use super::*;
use gpui::{point, px, size};

fn rect(y: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(0.), px(y)), size(px(100.), px(20.)))
}

#[test]
fn normal_nodes_are_not_admitted_and_oof_slices_are_not_inherited() {
    let mut snapshot = Snapshot::default();
    snapshot
        .families
        .extend([(10, 10), (11, 10), (20, 20), (21, 20)]);
    snapshot.normal.extend([(10, rect(0.)), (20, rect(20.))]);
    let mut admission = Admission::new();
    let source = point(px(0.), px(20.));
    admission
        .insert(
            20,
            PaintSegmentId(0),
            source,
            point(px(0.), px(20.)),
            rect(20.),
            rect(20.),
            true,
        )
        .unwrap();
    admission
        .insert(
            20,
            PaintSegmentId(1),
            source,
            point(px(100.), px(0.)),
            rect(20.),
            rect(20.),
            false,
        )
        .unwrap();
    let routing = admission.finish(&snapshot);
    let (hit_routes, hits) = &routing.hits;
    assert_eq!(hit_routes.len(), 2);
    assert!(hits.nodes[&10].is_empty() && hits.nodes[&11].is_empty());
    assert_eq!(hits.nodes[&21].len(), 2);
    let (paint_routes, paint) = &routing.paint;
    // Only the segment without a legacy copy is admitted by the source record.
    assert_eq!(paint_routes.len(), 1);
    assert_eq!(paint_routes[0].id, PaintSegmentId(1));
    assert_eq!(paint.nodes[&21].len(), 1);
    assert!(paint.boxes[&10].is_empty());
    let own = &paint.nested[&20];
    assert_eq!(&*own.bare, &[PaintSegmentId(0)]);
    // The orphan placement keeps source coordinates inside the own capture.
    let orphan = own
        .routes
        .iter()
        .find(|r| r.id == PaintSegmentId(1))
        .unwrap();
    assert_eq!(orphan.destination, orphan.source);
}

#[test]
fn duplicate_segment_ordinal_is_impossible_and_ordinals_are_per_segment() {
    let mut admission = Admission::new();
    for family in [1, 2] {
        admission
            .insert(
                family,
                PaintSegmentId(3),
                point(px(0.), px(0.)),
                point(px(0.), px(0.)),
                rect(0.),
                rect(0.),
                false,
            )
            .unwrap();
    }
    let routing = admission.finish(&Snapshot::default());
    let ordinals: Vec<_> = routing.hits.0.iter().map(|r| (r.id, r.ordinal)).collect();
    assert_eq!(ordinals, [(PaintSegmentId(3), 0), (PaintSegmentId(3), 1)]);
}
