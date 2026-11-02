//! Every measured-layout entry point runs its callback in the requesting
//! element's inherited text style and rem size, though Taffy calls it during
//! the root's layout after those scopes were popped.

use gpui::{
    AvailableSpace, Display, FlexDirection, LayoutId, Pixels, Style, TextStyleRefinement, Window,
    point, px, size,
};
use std::cell::Cell;
use std::rc::Rc;

type Seen = Rc<Cell<Vec<(Pixels, Pixels)>>>;

fn note(seen: &Seen, window: &Window) {
    let mut calls = seen.take();
    calls.push((
        window.text_style().font_size.to_pixels(px(16.)),
        window.rem_size(),
    ));
    seen.set(calls);
}

fn leaves(window: &mut Window, seen: &Seen) -> Vec<LayoutId> {
    let fixed = || size(px(10.), px(10.));
    let s = seen.clone();
    let a = window.request_measured_layout(Style::default(), move |_, _, w, _| {
        note(&s, w);
        fixed()
    });
    let s = seen.clone();
    let b = window.request_measured_layout_with_baseline(Style::default(), move |_, _, w, _| {
        note(&s, w);
        (fixed(), None)
    });
    let s = seen.clone();
    let c = window.request_measured_layout_with_baselines(Style::default(), move |_, _, w, _| {
        note(&s, w);
        (fixed(), None, None)
    });
    let s = seen.clone();
    let d = window.request_measured_layout_with_box(Style::default(), move |_, _, _, w, _| {
        note(&s, w);
        fixed()
    });
    let s = seen.clone();
    let e =
        window.request_measured_layout_with_line_baselines(Style::default(), move |_, _, w, _| {
            note(&s, w);
            (fixed(), None, None, Vec::new())
        });
    let s = seen.clone();
    let f = window.request_measured_layout_with_physical_baselines(
        Style::default(),
        move |_, _, w, _| {
            note(&s, w);
            gpui::MeasuredContent::new(fixed())
        },
    );
    vec![a, b, c, d, e, f]
}

#[gpui::test]
fn measure_callbacks_run_in_the_requesting_elements_scope(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            let seen: Seen = Rc::default();
            let big = TextStyleRefinement {
                font_size: Some(px(31.).into()),
                ..Default::default()
            };
            let kids = window.with_rem_size(Some(px(23.)), |window| {
                window.with_text_style(Some(big), |window| leaves(window, &seen))
            });
            let root = window.request_layout(
                Style {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    size: size(px(300.).into(), px(200.).into()),
                    ..Style::default()
                },
                kids,
                app,
            );
            let outside = (
                window.text_style().font_size.to_pixels(px(16.)),
                window.rem_size(),
            );
            assert_ne!(outside, (px(31.), px(23.)));
            let space = size(
                AvailableSpace::Definite(px(300.)),
                AvailableSpace::Definite(px(200.)),
            );
            window.compute_layout(root, space, app);
            let seen = seen.take();
            assert!(seen.len() >= 6, "{seen:?}");
            assert!(seen.iter().all(|s| *s == (px(31.), px(23.))), "{seen:?}");
            // The window's own scope is back after layout.
            let after = (
                window.text_style().font_size.to_pixels(px(16.)),
                window.rem_size(),
            );
            assert_eq!(after, outside);
            gpui::div()
        },
    );
}
