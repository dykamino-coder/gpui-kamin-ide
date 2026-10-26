//! Отклонение stale записи и восстановление полного состояния при abort.

use super::*;
use gpui::{
    AppContext, ContentMask, PaintError, PaintGroupDescriptor, PaintPlan, PaintRecordError,
    PaintScopeDescriptor, TransformationMatrix, canvas,
};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn invalid_placement_or_later_record_is_rejected_before_destination_writes() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, record) = window
                    .record_paint(extent, cx, |window, _| quad(window, rect(1., 2., 10., 10.)))
                    .unwrap();
                let plan = PaintPlan::record(record);
                let (_, output) = window
                    .record_paint(extent, cx, |window, _| {
                        for bad in [
                            placement(f32::NAN, 0., extent),
                            placement(0., 0., rect(0., 0., -1., 10.)),
                        ] {
                            assert_eq!(
                                window.paint_plan(&plan, bad).err(),
                                Some(PaintRecordError::InvalidGeometry)
                            );
                        }
                        let invalid = PaintPlan::sequence([
                            plan.clone(),
                            PaintPlan::scope(
                                PaintScopeDescriptor::Group(PaintGroupDescriptor {
                                    bounds: extent,
                                    opacity: f32::NAN,
                                    ..Default::default()
                                }),
                                plan.clone(),
                            ),
                        ]);
                        assert_eq!(
                            window.paint_plan(&invalid, placement(0., 0., extent)).err(),
                            Some(PaintRecordError::InvalidGeometry)
                        );
                        window.set_scale_factor(2.);
                        assert_eq!(
                            window.paint_plan(&plan, placement(0., 0., extent)).err(),
                            Some(PaintRecordError::ScaleMismatch)
                        );
                        window.set_scale_factor(1.);
                        window.paint_plan(&plan, placement(0., 0., extent)).unwrap();
                    })
                    .unwrap();
                assert_eq!(output.scene().quads.len(), 1);
                assert!(output.scene().groups.is_empty());
            },
        )
    });
}

#[test]
fn wrong_window_and_completed_draw_do_not_extend_record_lifetime() {
    let mut cx = gpui::TestAppContext::single();
    let mut first = cx.add_empty_window().clone();
    let second = cx
        .add_empty_window()
        .update(|window, _| window.window_handle());
    let saved = Rc::new(RefCell::new(None));
    let output = saved.clone();
    first.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
        canvas(
            |_, _, _| (),
            move |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, record) = window
                    .record_paint(extent, cx, |window, _| quad(window, rect(1., 2., 10., 10.)))
                    .unwrap();
                let plan = PaintPlan::record(record);
                cx.update_window(second, |_, other, _| {
                    assert_eq!(
                        other.paint_plan(&plan, placement(0., 0., extent)).err(),
                        Some(PaintRecordError::Identity(PaintError::WrongWindow))
                    );
                })
                .unwrap();
                *output.borrow_mut() = Some(plan);
            },
        )
    });
    let plan = saved.borrow_mut().take().unwrap();
    first.draw(point(px(0.), px(0.)), size(px(100.), px(100.)), |_, _| {
        canvas(
            |_, _, _| (),
            move |_, (), window, _| {
                assert_eq!(
                    window
                        .paint_plan(&plan, placement(0., 0., rect(0., 0., 90., 90.)))
                        .err(),
                    Some(PaintRecordError::Identity(PaintError::ExpiredPass))
                );
            },
        )
    });
    cx.quit();
}

#[test]
fn capture_unwind_restores_caller_and_invalidates_reusable_commands() {
    draw(|| {
        canvas(
            |_, _, _| (),
            |_, (), window, cx| {
                let extent = rect(0., 0., 90., 90.);
                let (_, record) = window
                    .record_paint(extent, cx, |window, _| quad(window, rect(1., 2., 10., 10.)))
                    .unwrap();
                let mask = window.content_mask();
                let frame = window.raster_frame();
                let scope = window.paint_scope_id();
                let host = Rc::new(7u32);
                window.with_render_context(Some(host.clone()), |window| {
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        let _ = window.record_paint(rect(3., 4., 10., 10.), cx, |window, _| {
                            window.with_transformation(
                                TransformationMatrix {
                                    translation: [8., 9.],
                                    ..TransformationMatrix::unit()
                                },
                                |window| {
                                    window.with_content_mask(
                                        Some(ContentMask {
                                            bounds: rect(5., 6., 7., 8.),
                                        }),
                                        |_| {
                                            panic!("source paint abort");
                                        },
                                    );
                                },
                            );
                        });
                    }));
                    assert!(result.is_err());
                    assert!(Rc::ptr_eq(&window.render_context::<u32>().unwrap(), &host));
                });
                assert_eq!(window.content_mask(), mask);
                assert_eq!(window.raster_frame(), frame);
                assert_eq!(
                    window.current_transformation(),
                    TransformationMatrix::unit()
                );
                assert!(window.paint_scope_id() == scope);
                assert!(!window.paint_pass_id().is_live());
                assert_eq!(
                    window
                        .paint_plan(&PaintPlan::record(record), placement(0., 0., extent))
                        .err(),
                    Some(PaintRecordError::Identity(PaintError::ExpiredPass))
                );
            },
        )
    });
}
