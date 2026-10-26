//! Повторное размещение команд не повторяет AnyElement или фазы хозяина.

use super::*;
use crate::hooks::{Hooks, IntoHooked};
use gpui::{AnyElement, App, ContentMask, LayoutId, PaintPlan, TransformationMatrix, canvas};
use std::{cell::Cell, rc::Rc};

struct Source(Rc<Cell<[usize; 3]>>);

impl Source {
    fn count(&self, index: usize) {
        let mut counts = self.0.get();
        counts[index] += 1;
        self.0.set(counts);
    }
}

impl Hooks for Source {
    type Layout = ();
    type Prepaint = ();
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        self.count(0);
        (window.request_layout(Default::default(), [], cx), ())
    }
    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        self.count(1);
        assert_eq!(
            window
                .record_paint(rect(0., 0., 10., 10.), cx, |_, _| panic!(
                    "paint callback in prepaint"
                ))
                .err(),
            Some(gpui::PaintRecordError::NotPainting)
        );
    }
    fn paint(&mut self, _: Bounds<Pixels>, _: &mut (), _: &mut (), w: &mut Window, _: &mut App) {
        self.count(2);
        quad(w, rect(70., 3., 10., 10.));
    }
}

struct Consumer(AnyElement);

impl Hooks for Consumer {
    type Layout = ();
    type Prepaint = ();
    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.0.request_layout(window, cx), ())
    }
    fn prepaint(&mut self, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        self.0.prepaint(window, cx);
    }
    fn paint(
        &mut self,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let (_, output) = window
            .record_paint(rect(0., 0., 100., 100.), cx, |window, cx| {
                window.with_content_mask(
                    Some(ContentMask {
                        bounds: rect(0., 0., 20., 20.),
                    }),
                    |window| {
                        let (_, record) = window
                            .record_paint(rect(0., 0., 100., 100.), cx, |window, cx| {
                                self.0.paint(window, cx);
                            })
                            .unwrap();
                        assert_eq!(
                            record.scene().quads.len(),
                            1,
                            "source is not culled by first fragment"
                        );
                        assert!(record.same_record(&record.clone()));
                        let plan = PaintPlan::record(record);
                        for x in [-65., -55.] {
                            let scope = window
                                .paint_plan(&plan, placement(x, 0., rect(0., 0., 20., 20.)))
                                .unwrap();
                            assert!(!scope.is_live());
                        }
                    },
                );
            })
            .unwrap();
        let quads = &output.scene().quads;
        assert_eq!(quads.len(), 2);
        assert_eq!(quads[0].transformation.translation, [-65., 0.]);
        assert_eq!(quads[1].transformation.translation, [-55., 0.]);
        assert_eq!(quads[0].bounds.origin.x.0, 70.);
        assert_eq!(quads[1].content_mask.bounds.size.width.0, 20.);
    }
}

#[test]
fn source_element_is_consumed_once_and_replayed_in_two_fragments() {
    let counts = Rc::new(Cell::new([0; 3]));
    draw(|| Consumer(Source(counts.clone()).hooked()).hooked());
    assert_eq!(counts.get(), [1, 1, 1]);
}

#[test]
fn fractional_scale_maps_resolved_transform_once_and_restores_caller() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                window.set_scale_factor(1.25);
                let transform = TransformationMatrix {
                    translation: [2.25, 3.75],
                    ..TransformationMatrix::unit()
                };
                let (_, record) = window
                    .with_transformation(transform, |window| {
                        window.record_paint(rect(0., 0., 100., 100.), cx, |window, _| {
                            quad(window, rect(1., 2., 9., 8.))
                        })
                    })
                    .unwrap();
                let before = window.snapshot_paint_state();
                let (_, output) = window
                    .record_paint(rect(0., 0., 100., 100.), cx, |window, _| {
                        window.with_transformation(
                            TransformationMatrix {
                                translation: [300., 400.],
                                ..TransformationMatrix::unit()
                            },
                            |window| {
                                window
                                    .paint_plan(
                                        &PaintPlan::record(record.clone()),
                                        placement(11.5, 12.5, rect(0., 0., 90., 90.)),
                                    )
                                    .unwrap();
                            },
                        );
                    })
                    .unwrap();
                let source = &record.scene().quads[0];
                let target = &output.scene().quads[0];
                assert_eq!(source.bounds, target.bounds);
                assert_eq!(source.background, target.background);
                assert_eq!(
                    target.transformation.translation,
                    [
                        source.transformation.translation[0] + 11.5 * 1.25,
                        source.transformation.translation[1] + 12.5 * 1.25
                    ]
                );
                window.with_paint_state(before, |_| ()).unwrap();
            },
        )
    });
}
