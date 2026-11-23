//! Float overflow survives independent measurement without increasing normal-flow height.

use super::*;

fn floating(build: Build) -> Kid {
    let mut child = kid(
        Kind::Float {
            side: -1,
            clear: None,
            shrink: false,
            letter: false,
            own_layout: false,
        },
        Default::default(),
        Default::default(),
    );
    child.build = build;
    child
}

#[gpui::test]
fn float_overflow_survives_a_cached_plan_without_entering_normal_flow(
    cx: &mut gpui::TestAppContext,
) {
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            let builds = Rc::new(Cell::new(0));
            let count = builds.clone();
            let child = floating(Rc::new(move |_, _, _, _, _| {
                count.set(count.get() + 1);
                div().w(px(40.)).h(px(180.)).into_any_element()
            }));
            let flow = BandFlow::new(vec![child], false, &block_cx());
            let plans = flow.plan.clone();
            let mut element = flow.hooked();
            let root = element.request_layout(window, app);
            for width in [100., 120., 100.] {
                window.compute_layout(
                    root,
                    size(
                        AvailableSpace::Definite(px(width)),
                        AvailableSpace::MaxContent,
                    ),
                    app,
                );
                assert_eq!(window.layout_size_unrounded(root), size(px(width), px(0.)));
                assert_eq!(window.layout_content_size_unrounded(root).height, px(180.));
                assert_eq!(plans.get(width).unwrap().height, 0.);
            }
            assert_eq!(
                builds.get(),
                2,
                "returning to a retained plan preserves its overflow"
            );
            div()
        },
    );
}

#[gpui::test]
fn float_child_clipping_limits_only_the_propagated_overflow(cx: &mut gpui::TestAppContext) {
    cx.add_empty_window().draw(
        point(px(0.), px(0.)),
        size(px(100.), px(100.)),
        |window, app| {
            for clipped in [false, true] {
                let child = floating(Rc::new(move |_, _, _, _, _| {
                    let outer = div()
                        .relative()
                        .w(px(40.))
                        .h(px(30.))
                        .child(div().absolute().w(px(40.)).h(px(180.)));
                    if clipped {
                        outer.overflow_hidden().into_any_element()
                    } else {
                        outer.into_any_element()
                    }
                }));
                let mut element = BandFlow::new(vec![child], false, &block_cx()).hooked();
                let root = element.request_layout(window, app);
                window.compute_layout(
                    root,
                    size(
                        AvailableSpace::Definite(px(100.)),
                        AvailableSpace::MaxContent,
                    ),
                    app,
                );
                assert_eq!(window.layout_size_unrounded(root).height, px(0.));
                assert_eq!(
                    window.layout_content_size_unrounded(root).height,
                    px(if clipped { 30. } else { 180. })
                );
            }
            div()
        },
    );
}

#[test]
fn plan_overflow_uses_physical_slot_placement_in_vertical_modes() {
    let plan = Plan {
        width: 100.,
        height: 60.,
        slots: vec![Slot {
            x: 10.,
            y: 20.,
            i: 30.,
            b: 15.,
            overflow: Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(45.), px(130.)),
            },
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_eq!(
        plan.measured(None, false).overflow,
        Some(size(px(100.), px(150.)))
    );
    assert_eq!(
        plan.measured(Some(true), false).overflow,
        Some(size(px(70.), px(140.)))
    );
    assert_eq!(
        plan.measured(Some(false), true).overflow,
        Some(size(px(65.), px(190.)))
    );
    // Switching the physical writing axis must not reuse the previous extent.
    assert_eq!(
        plan.measured(Some(false), false).overflow,
        Some(size(px(65.), px(140.)))
    );
    assert_eq!(
        plan.measured(None, false).overflow,
        Some(size(px(100.), px(150.)))
    );
}
