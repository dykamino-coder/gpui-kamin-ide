//! Потребитель сохраняет общую краску декораций и CSS-слот табличных полос.

use super::*;
use crate::layout::fragment::{
    decoration::Decoration, paint_source::SourceSlices, types::StackAxis,
};
use crate::paint::coordinator::segments as consumer;
use gpui::{
    BoxShadow, Corners, Edges, PaintPlan, PaintSegment, PaintSegmentId, PaintSegmentSlice, Style,
    canvas,
};
use std::{cell::Cell, rc::Rc};

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
                            consumer::node(7, None, window, |window| {
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
                    .unwrap();
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

#[test]
fn table_source_slot_stays_between_oof_siblings_inside_one_ancestor_effect() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let calls = Rc::new(Cell::new(0));
                let observed = calls.clone();
                let part = slice(1, 0.);
                let mut slices = SourceSlices::default();
                for id in [1, 2, 3] {
                    slices.nodes.insert(id, Rc::from([part]));
                }
                slices.slots.insert(
                    8,
                    Rc::new(move |window, _| {
                        observed.set(observed.get() + 1);
                        window.with_paint_segment_slices(Rc::from([part]), |window| {
                            window.paint_quad(fill(rect(4., 4., 20., 20.), rgb(0x00ff00)));
                        })?;
                        Ok(())
                    }),
                );
                let records = consumer::record(
                    extent,
                    Rc::from([part.placement]),
                    slices,
                    window,
                    cx,
                    |window, cx| {
                        consumer::node(1, None, window, |window| {
                            window.paint_group(
                                extent,
                                Corners::default(),
                                3.,
                                0.5,
                                0,
                                &[],
                                None,
                                None,
                                |window| {
                                    consumer::node(2, None, window, |window| {
                                        window.paint_quad(fill(
                                            rect(4., 4., 20., 20.),
                                            rgb(0xff0000),
                                        ));
                                    });
                                    consumer::slot(8, window, cx);
                                    consumer::node(3, None, window, |window| {
                                        window.paint_quad(fill(
                                            rect(4., 4., 20., 20.),
                                            rgb(0x0000ff),
                                        ));
                                    });
                                },
                            );
                        })
                    },
                )
                .unwrap();
                assert_eq!(calls.get(), 1);
                let record = records.get(PaintSegmentId(1)).unwrap();
                let scene = record.scene();
                assert_eq!(scene.groups.len(), 1);
                assert_eq!(scene.surfaces.len(), 1);
                assert_eq!(scene.surfaces[0].opacity, 0.5);
                assert_eq!(scene.groups[0].blur_radius, 3.);
                let quads = &scene.groups[0].scene.quads;
                assert_eq!(quads.len(), 3);
                for (quad, color) in quads.iter().zip([0xff0000, 0x00ff00, 0x0000ff]) {
                    assert_eq!(
                        format!("{:?}", quad.background),
                        format!("{:?}", gpui::Background::from(rgb(color)))
                    );
                }
                assert!(quads[0].order < quads[1].order);
                assert!(quads[1].order < quads[2].order);
                for x in [0., 30.] {
                    window
                        .paint_plan(&PaintPlan::record(record.clone()), placement(x, 0., extent))
                        .unwrap();
                }
                assert_eq!(calls.get(), 1);
            },
        )
    });
}
