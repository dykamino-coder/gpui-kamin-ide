//! A record landed moved and clipped, once per column of a flow: the node it
//! opened inside itself lands with the column that shows it.

use super::super::*;
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
    let operations = &window.next_frame.scene.paint_operations;
    let quad = |op: &PaintOperation| match op {
        PaintOperation::Primitive(p @ Primitive::Quad(_)) => Some(crate::scene::placed_bounds(p)),
        _ => None,
    };
    operations.iter().filter_map(quad).collect()
}

#[crate::test]
fn a_node_inside_a_record_lands_with_the_moved_column_that_shows_it(cx: &mut TestAppContext) {
    cx.add_window(|_, _| Empty)
        .update(cx, |_, window, cx| {
            window.scroll_interest_margin = px(100.);
            let (outer, inner) = (ScrollHandle::new(), ScrollHandle::new());
            outer.set_max_offset(point(px(0.), px(10_000.)));
            let (outer_id, inner_id) = (ScrollNodeId::of(&outer), ScrollNodeId::of(&inner));
            window.invalidator.set_phase(DrawPhase::Prepaint);
            let (_, prepaint) = window.record_retained_prepaint(|window| {
                window.with_scroll_node_prepaint(&outer, rect(0., 0., 220., 50.), |window| {
                    window.with_scroll_node_prepaint(&inner, rect(0., 0., 100., 120.), |_| {});
                });
            });
            window.invalidator.set_phase(DrawPhase::Paint);
            let extent = rect(0., 0., 100., 120.);
            let (_, paint) = window.with_paint_pass(|window| {
                window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(outer_id, |window| {
                        let (_, record) = window
                            .record_paint(extent, cx, |window, _| {
                                window.with_scroll_node_paint(inner_id, |window| {
                                    let scale = window.scale_factor();
                                    // In the second column's part, below the outer clip: dormant.
                                    window.insert_scene_primitive(Quad {
                                        bounds: rect(10., 110., 10., 10.).scale(scale),
                                        content_mask: window.content_mask().scale(scale),
                                        ..Default::default()
                                    });
                                });
                            })
                            .unwrap();
                        let plan = PaintPlan::record(record);
                        // Two columns of the flow: its top half in place, its bottom half beside it.
                        let first = PaintPlacement { source: point(px(0.), px(0.)), destination: point(px(0.), px(0.)), clip: rect(0., 0., 100., 60.) };
                        let second = PaintPlacement { source: point(px(0.), px(60.)), destination: point(px(110.), px(0.)), clip: rect(110., 0., 100., 60.) };
                        window.paint_plan(&plan, first).unwrap();
                        window.paint_plan(&plan, second).unwrap();
                    });
                })
            });
            let frame_space = window.next_frame.scene.scroll.space;
            let node = window.next_frame.scroll_nodes.iter().find(|node| node.id == inner_id).unwrap();
            assert_eq!((node.painted_in, node.torn), (Some(frame_space), false), "the inner node lands with the second column");
            assert_eq!(node.chain, rect(110., 0., 100., 60.), "its mask moved and clipped as the column");
            std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
            window.next_frame.clear();
            outer.set_offset(point(px(0.), px(-40.)));
            let plan = window
                .plan_scrolled_replay(&prepaint, &paint, None)
                .expect("the moved record's node replays scrolled");
            window.invalidator.set_phase(DrawPhase::Prepaint);
            window.replay_retained_prepaint_scrolled(&prepaint, &plan);
            window.invalidator.set_phase(DrawPhase::Paint);
            window.replay_retained_paint_scrolled(&paint, &plan);
            let scale = window.scale_factor();
            assert_eq!(quads(window), vec![rect(120., 10., 10., 10.).scale(scale)], "the step shows the quad in the column");
        })
        .unwrap();
}