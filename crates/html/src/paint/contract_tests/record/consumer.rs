//! Потребитель сохраняет общую краску декораций и CSS-слот табличных полос.

use super::*;
use crate::layout::fragment::{
    decoration::Decoration, paint_source::SourceSlices, types::StackAxis,
};
use crate::paint::coordinator::segments as consumer;
use gpui::{
    BoxShadow, Corners, Edges, PaintSegment, PaintSegmentId, PaintSegmentSlice, Style, canvas,
};
use std::rc::Rc;

fn route(id: u64, y: f32) -> PaintSegment {
    PaintSegment {
        id: PaintSegmentId(id),
        ordinal: 0,
        source: point(px(0.), px(y)),
        destination: point(px((id - 1) as f32 * 40.), px(0.)),
    }
}

fn slice(id: u64, y: f32) -> PaintSegmentSlice {
    PaintSegmentSlice {
        placement: route(id, y),
        source_clip: rect(-20., y, 100., 30.),
    }
}

fn style() -> Style {
    Style {
        background: Some(rgb(0x336699).into()),
        border_color: Some(rgb(0x992244).into()),
        border_widths: Edges::all(px(2.).into()),
        corner_radii: Corners::all(px(6.)).map(|v| (*v).into()),
        box_shadow: vec![BoxShadow::new(px(2.), px(3.), rgb(0).into()).blur_radius(px(4.))],
        ..Default::default()
    }
}

#[test]
fn fragment_box_override_matches_common_background_border_radius_and_shadow_painter() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(-20., -20., 140., 140.);
                let whole = rect(0., 0., 30., 70.);
                let parts = [slice(1, 5.), slice(2, 35.)];
                let routes: Rc<[PaintSegment]> = Rc::from([parts[0].placement, parts[1].placement]);
                for clone in [false, true] {
                    let boxes: Rc<[_]> = parts
                        .iter()
                        .map(|part| {
                            Decoration::for_slice(
                                whole,
                                *part,
                                clone.then_some((StackAxis::Horizontal, 5., 5.)),
                            )
                            .unwrap()
                        })
                        .collect();
                    let mut slices = SourceSlices::default();
                    slices.nodes.insert(7, Rc::from(parts));
                    slices.boxes.insert(7, boxes.clone());
                    let actual = consumer::record(
                        extent,
                        routes.clone(),
                        slices,
                        window,
                        cx,
                        |window, cx| {
                            consumer::node(7, None, window, cx, |window, cx| {
                                style().paint_with_box(
                                    whole,
                                    Some(&consumer::box_painter(7)),
                                    window,
                                    cx,
                                    |_, _| {},
                                );
                            });
                        },
                    )
                    .unwrap()
                    .0;
                    let (_, expected) = window
                        .record_paint_segments(extent, routes.clone(), cx, |window, cx| {
                            for (index, geometry) in boxes.iter().enumerate() {
                                let bounds = if clone {
                                    rect(0., 30. * index as f32, 30., 40.)
                                } else {
                                    whole
                                };
                                assert_eq!(geometry.bounds, bounds);
                                window
                                    .with_paint_segment_slices(
                                        Rc::from([geometry.slice]),
                                        |window| {
                                            style().paint_box(bounds, window, cx);
                                        },
                                    )
                                    .unwrap();
                            }
                        })
                        .unwrap();
                    for id in [PaintSegmentId(1), PaintSegmentId(2)] {
                        let a = actual.get(id).unwrap().scene();
                        let b = expected.get(id).unwrap().scene();
                        assert!(!a.quads.is_empty());
                        assert_eq!(a.shadows.len(), 1);
                        assert_eq!(format!("{:?}", a.quads), format!("{:?}", b.quads));
                        assert_eq!(format!("{:?}", a.shadows), format!("{:?}", b.shadows));
                    }
                }
            },
        )
    });
}
