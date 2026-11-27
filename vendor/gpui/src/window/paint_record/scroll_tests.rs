//! A paint record that lands under a scroll node replays scrolled: what it
//! records under the node open before it began stays unmarked and is never
//! made dormant there (`ScrollBook::record_base`), and its copy is marked
//! under the node where it lands (`Window::with_copies_marked`).

use super::segments::{PaintSegment, PaintSegmentId};
use super::*;
use crate::scene::{PaintOperation, Primitive};
use crate::{Context, IntoElement, Quad, Render, ScrollHandle, ScrollNodeId, TestAppContext, div};

struct Empty;
impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
}

/// The placed quads of the next frame's scene, drawn ones only.
fn quads(window: &Window) -> Vec<Bounds<ScaledPixels>> {
    let scene = &window.next_frame.scene;
    scene
        .paint_operations
        .iter()
        .filter_map(|op| match op {
            PaintOperation::Primitive(p @ Primitive::Quad(_)) => {
                Some(crate::scene::placed_bounds(p))
            }
            _ => None,
        })
        .collect()
}

#[crate::test]
fn a_record_hidden_below_the_node_clip_replays_into_view(cx: &mut TestAppContext) {
    cx.add_window(|_, _| Empty)
        .update(cx, |_, window, cx| {
            window.scroll_interest_margin = px(100.);
            let handle = ScrollHandle::new();
            handle.set_max_offset(point(px(0.), px(10_000.)));
            let id = ScrollNodeId::of(&handle);
            window.invalidator.set_phase(DrawPhase::Prepaint);
            let (_, prepaint) = window.record_retained_prepaint(|window| {
                window.with_scroll_node_prepaint(&handle, rect(0., 0., 100., 50.), |_| {});
            });
            window.invalidator.set_phase(DrawPhase::Paint);
            // A record belongs to a live paint pass (`paint_scope::Identity`).
            let (_, paint) = window.with_paint_pass(|window| {
                window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(id, |window| {
                        let extent = rect(0., 0., 100., 120.);
                        // Inside the node's interest rect, below its clip: hidden now.
                        // One segment of a recorded flow (a column), frozen out of
                        // the record's scene (`Scene::freeze_segment`).
                        let segment = PaintSegment {
                            id: PaintSegmentId(1),
                            ordinal: 0,
                            source: point(px(0.), px(0.)),
                            destination: point(px(0.), px(0.)),
                        };
                        let (_, records) = window
                            .record_paint_segments(
                                extent,
                                std::rc::Rc::from([segment]),
                                cx,
                                |window, _| {
                                    let scale = window.scale_factor();
                                    window.insert_scene_primitive(Quad {
                                        bounds: rect(10., 80., 10., 10.).scale(scale),
                                        content_mask: window.content_mask().scale(scale),
                                        ..Default::default()
                                    });
                                },
                            )
                            .unwrap();
                        let record = records.get(PaintSegmentId(1)).unwrap().clone();
                        let placement = PaintPlacement {
                            source: point(px(0.), px(0.)),
                            destination: point(px(0.), px(0.)),
                            clip: extent,
                        };
                        window
                            .paint_plan(&PaintPlan::record(record), placement)
                            .unwrap();
                    });
                })
            });
            std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
            window.next_frame.clear();
            handle.set_offset(point(px(0.), px(-40.)));
            let plan = window
                .plan_scrolled_replay(&prepaint, &paint, None)
                .expect("a landed record replays scrolled");
            window.invalidator.set_phase(DrawPhase::Prepaint);
            window.replay_retained_prepaint_scrolled(&prepaint, &plan);
            window.invalidator.set_phase(DrawPhase::Paint);
            window.replay_retained_paint_scrolled(&paint, &plan);
            let scale = window.scale_factor();
            assert_eq!(
                quads(window),
                vec![rect(10., 40., 10., 10.).scale(scale)],
                "the step shows the quad"
            );
        })
        .unwrap();
}

/// A node opened inside a record paints into the record's scene: landed
/// where it was recorded, the record's copy keeps the node's marks and the
/// node takes its range in the frame's scene, so a step of the outer node
/// replays the frame (the inner node moves with it) instead of building.
#[crate::test]
fn a_node_painted_inside_a_record_lands_with_it(cx: &mut TestAppContext) {
    cx.add_window(|_, _| Empty)
        .update(cx, |_, window, cx| {
            window.scroll_interest_margin = px(100.);
            let (outer, inner) = (ScrollHandle::new(), ScrollHandle::new());
            outer.set_max_offset(point(px(0.), px(10_000.)));
            let (outer_id, inner_id) = (ScrollNodeId::of(&outer), ScrollNodeId::of(&inner));
            window.invalidator.set_phase(DrawPhase::Prepaint);
            let (_, prepaint) = window.record_retained_prepaint(|window| {
                window.with_scroll_node_prepaint(&outer, rect(0., 0., 100., 50.), |window| {
                    window.with_scroll_node_prepaint(&inner, rect(0., 0., 100., 120.), |_| {});
                });
            });
            window.invalidator.set_phase(DrawPhase::Paint);
            let (_, paint) = window.with_paint_pass(|window| {
                window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(outer_id, |window| {
                        let extent = rect(0., 0., 100., 120.);
                        let (_, record) = window
                            .record_paint(extent, cx, |window, _| {
                                window.with_scroll_node_paint(inner_id, |window| {
                                    let scale = window.scale_factor();
                                    // Below the outer clip: dormant under the inner node.
                                    window.insert_scene_primitive(Quad {
                                        bounds: rect(10., 80., 10., 10.).scale(scale),
                                        content_mask: window.content_mask().scale(scale),
                                        ..Default::default()
                                    });
                                });
                            })
                            .unwrap();
                        let placement = PaintPlacement {
                            source: point(px(0.), px(0.)),
                            destination: point(px(0.), px(0.)),
                            clip: extent,
                        };
                        window.paint_plan(&PaintPlan::record(record), placement).unwrap();
                    });
                })
            });
            let frame_space = window.next_frame.scene.scroll.space;
            let node = window.next_frame.scroll_nodes.iter().find(|node| node.id == inner_id).unwrap();
            assert_eq!(node.painted_in, Some(frame_space), "the inner node lands in the frame's scene");
            std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
            window.next_frame.clear();
            outer.set_offset(point(px(0.), px(-40.)));
            let plan = window
                .plan_scrolled_replay(&prepaint, &paint, None)
                .expect("the record's node replays scrolled");
            window.invalidator.set_phase(DrawPhase::Prepaint);
            window.replay_retained_prepaint_scrolled(&prepaint, &plan);
            window.invalidator.set_phase(DrawPhase::Paint);
            window.replay_retained_paint_scrolled(&paint, &plan);
            let scale = window.scale_factor();
            assert_eq!(quads(window), vec![rect(10., 40., 10., 10.).scale(scale)], "the step shows the quad");
        })
        .unwrap();
}