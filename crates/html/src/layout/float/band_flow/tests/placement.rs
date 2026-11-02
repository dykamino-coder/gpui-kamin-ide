//! Прямой держатель сохраняет дробное физическое место во всех направлениях.

use super::*;
use gpui::RetainedLayout;

#[gpui::test]
fn prepared_holder_places_once_relative_to_its_containing_origin(cx: &mut gpui::TestAppContext) {
    for scale in [1., 2.] {
        for (vert, inline_up, left, top) in [
            (None, false, 23.25, 31.5),
            (Some(true), false, 48.5, 23.25),
            (Some(false), false, 31.5, 23.25),
            (Some(false), true, 31.5, 66.75),
        ] {
            let builds = Rc::new(Cell::new(0));
            let bounds = Rc::new(Cell::new(None));
            let child = kid(Kind::Flow, builds.clone(), bounds.clone());
            let window = cx.add_empty_window();
            window.simulate_scale_factor_change(scale);
            window.draw(
                point(px(0.), px(0.)),
                size(px(150.), px(150.)),
                |window, app| {
                    let mut engine = RetainedLayout::default();
                    let retained = engine.clone();
                    let space = size(px(100.).into(), AvailableSpace::MaxContent);
                    let prepared = window.with_retained_layout(&mut engine, |window| {
                        let mut element = (child.build)(100., 100., None, None, None);
                        let measured = element.layout_as_root_unrounded(space, window, app);
                        prepared::Prepared::new(
                            element,
                            retained,
                            window.element_id_context(),
                            space,
                            measured,
                        )
                    });
                    let slot = Slot {
                        prepared,
                        x: 23.25,
                        y: 31.5,
                        i: if vert.is_some() { 10. } else { 20. },
                        b: if vert.is_some() { 20. } else { 10. },
                        ..Default::default()
                    };
                    let context = block_cx();
                    let at = holders::Placing {
                        paint: &context.deferred.paint,
                        build: &context.contracts.build,
                        cb: 100.,
                        height: 100.,
                        vert,
                        inline_up,
                    };
                    div().relative().w(px(150.)).h(px(150.)).child(
                        div()
                            .absolute()
                            .left(px(8.25))
                            .top(px(12.5))
                            .w(px(100.))
                            .h(px(100.))
                            .children(holders::holders(&[child], &[slot], &at, (5.25, 7.5))),
                    )
                },
            );
            let round = |v: f32| px((v * scale).round() / scale);
            assert_eq!(builds.get(), 1, "holder reuses the measured child");
            assert_eq!(
                bounds.get().expect("prepared child reaches prepaint"),
                Bounds::new(
                    point(round(8.25 + left - 5.25), round(12.5 + top - 7.5)),
                    size(px(20.), px(10.)),
                ),
                "vert={vert:?}, inline_up={inline_up}, scale={scale}",
            );
        }
    }
}
