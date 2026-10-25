//! Публичный путь Window → adapter: нормальные координаты и актуальность прохода.
use super::*;
use gpui::{AvailableSpace, Display, Edges, Style, point, px, relative, size};

fn tree(
    window: &mut Window,
    cx: &mut gpui::App,
    mut style: Style,
    shifted: bool,
) -> (LayoutId, Node) {
    let child = window.request_layout(
        Style {
            size: size(px(20.).into(), px(30.).into()),
            inset: if shifted {
                Edges {
                    left: relative(0.1).into(),
                    top: relative(0.25).into(),
                    ..Edges::all(gpui::Length::Auto)
                }
            } else {
                Edges::all(gpui::Length::Auto)
            },
            ..Style::default()
        },
        [],
        cx,
    );
    style.size = size(px(200.).into(), px(100.).into());
    if shifted {
        style.inset.left = px(3.).into();
        style.inset.top = px(5.).into();
    }
    let parent = window.request_layout(style, [child], cx);
    let root = window.request_layout(
        Style {
            display: Display::Block,
            ..Style::default()
        },
        [parent],
        cx,
    );
    (root, Node::capture(child, window))
}

fn solve(window: &mut Window, cx: &mut gpui::App, root: LayoutId, w: f32, h: f32) {
    window.compute_layout(
        root,
        size(
            AvailableSpace::Definite(px(w)),
            AvailableSpace::Definite(px(h)),
        ),
        cx,
    );
}

#[gpui::test]
fn relative_percentages_and_paint_placement_do_not_move_fragmentation(
    cx: &mut gpui::TestAppContext,
) {
    let cx = cx.add_empty_window();
    for scale in [1., 1.25, 1.5, 2.] {
        cx.simulate_scale_factor_change(scale);
        for style in [
            Style {
                display: Display::Block,
                ..Style::default()
            },
            Style {
                display: Display::Flex,
                ..Style::default()
            },
            Style {
                display: Display::Grid,
                ..Style::default()
            },
            Style {
                display: Display::Block,
                block_flow: Some([true, true, true]),
                ..Style::default()
            },
        ] {
            cx.draw(
                point(px(0.), px(0.)),
                size(px(400.), px(400.)),
                |window, app| {
                    let (root, plain) = tree(window, app, style.clone(), false);
                    solve(window, app, root, 300., 300.);
                    let expected = plain.bounds(window).unwrap();
                    let (root, shifted) = tree(window, app, style, true);
                    assert!(shifted.read(window).is_none());
                    solve(window, app, root, 300., 300.);
                    let geometry = shifted.read(window).unwrap();
                    assert_eq!(geometry.normal, expected);
                    assert_eq!(
                        geometry.normal.origin + geometry.relative_offset,
                        window.layout_origin_unrounded(shifted.id)
                    );
                    assert_ne!(geometry.relative_offset, point(px(0.), px(0.)));
                    window.set_layout_root_origin(root, point(px(100.), px(200.)));
                    window.set_layout_placed_origin(shifted.id, point(px(40.), px(60.)));
                    window.with_element_offset(point(px(50.), px(70.)), |window| {
                        assert_eq!(shifted.read(window), Some(geometry));
                    });
                    gpui::div()
                },
            );
        }
    }
}

#[gpui::test]
fn final_membership_dirty_ancestors_epochs_and_both_constraints(cx: &mut gpui::TestAppContext) {
    let cx = cx.add_empty_window();
    cx.draw(
        point(px(0.), px(0.)),
        size(px(400.), px(400.)),
        |window, app| {
            let (root, child) = tree(
                window,
                app,
                Style {
                    display: Display::Flex,
                    ..Style::default()
                },
                false,
            );
            let unlaid = window.request_layout(Style::default(), [], app);
            let unlaid = Node::capture(unlaid, window);
            solve(window, app, root, 300., 300.);
            assert!(unlaid.read(window).is_none());
            let first = child.read(window).unwrap();
            solve(window, app, root, 300., 300.);
            let repeated = child.read(window).unwrap();
            assert_ne!(first.space.epoch, repeated.space.epoch);
            assert_eq!(first.normal, repeated.normal);
            solve(window, app, root, 300., 200.);
            let shorter = child.read(window).unwrap();
            assert_ne!(shorter.space.available, repeated.space.available);
            solve(window, app, root, 250., 200.);
            assert_ne!(
                child.read(window).unwrap().space.available,
                shorter.space.available
            );
            window.set_flex_item_fragmentation_consumed(child.id, 1.);
            assert!(child.read(window).is_none());
            solve(window, app, root, 250., 200.);
            let resumed = child.bounds(window).unwrap();
            solve(window, app, child.id, 80., 100.);
            assert!(child.read(window).is_none());
            solve(window, app, root, 250., 200.);
            assert_eq!(child.bounds(window), Some(resumed));
            window.with_nested_layout(|window| {
                assert!(child.read(window).is_none());
                assert!(child.id_in(window).is_none());
            });
            assert!(child.read(window).is_some());
            gpui::div()
        },
    );
}
