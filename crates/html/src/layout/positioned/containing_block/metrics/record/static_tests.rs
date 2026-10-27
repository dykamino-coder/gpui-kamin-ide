//! Static rect принадлежит performed layout; скопированный замер не меняет исходный узел.
use super::*;
use gpui::{AvailableSpace, Edges, MeasuredContent, MeasuredStaticPosition, Style};

#[gpui::test]
fn performed_static_rect_survives_copy_measurement_and_rejects_dirty_ancestry(
    cx: &mut gpui::TestAppContext,
) {
    let cx = cx.add_empty_window();
    for scale in [1., 1.25, 2.] {
        cx.simulate_scale_factor_change(scale);
        cx.draw(
            point(px(0.), px(0.)),
            size(px(400.), px(400.)),
            |window, app| {
                let tree = window.layout_tree_id();
                let leaf = window.request_measured_layout_with_physical_baselines(
                    Style {
                        padding: Edges::all(px(3.).into()),
                        ..Default::default()
                    },
                    |known, available, _, _| {
                        let w = known.width.unwrap_or(match available.width {
                            AvailableSpace::Definite(w) => w,
                            _ => px(20.),
                        });
                        MeasuredContent {
                            static_positions: vec![MeasuredStaticPosition {
                                key: 7,
                                rect: Bounds {
                                    origin: point(w / 2., px(5.)),
                                    size: size(px(0.), px(10.)),
                                },
                                line: Bounds {
                                    origin: point(px(0.), px(5.)),
                                    size: size(w, px(10.)),
                                },
                            }],
                            ..MeasuredContent::new(size(w, px(20.)))
                        }
                    },
                );
                let node = Node::capture(leaf, window);
                let root = window.request_layout(
                    Style {
                        display: gpui::Display::Block,
                        ..Default::default()
                    },
                    [leaf],
                    app,
                );
                assert!(window.final_static_position(leaf, tree, 7).is_none());
                let available = size(
                    AvailableSpace::Definite(px(100.)),
                    AvailableSpace::Definite(px(80.)),
                );
                window.compute_layout(root, available, app);
                let initial = window.final_static_position(leaf, tree, 7).unwrap();
                assert_eq!(initial.rect.origin, point(px(50.), px(8.)));
                let mut copy = window.snapshot_layout_measurement(leaf);
                copy.measure(
                    size(
                        AvailableSpace::Definite(px(40.)),
                        AvailableSpace::MaxContent,
                    ),
                    window,
                    app,
                );
                assert_eq!(window.final_static_position(leaf, tree, 7), Some(initial));
                window.with_nested_layout(|window| {
                    assert!(node.id_in(window).is_none());
                    assert!(window.final_static_position(leaf, tree, 7).is_none());
                });
                window.compute_layout(root, available, app);
                assert_eq!(window.final_static_position(leaf, tree, 7), Some(initial));
                window.set_flex_item_fragmentation_consumed(leaf, 1.);
                assert!(window.final_static_position(leaf, tree, 7).is_none());
                gpui::div()
            },
        );
    }
}
