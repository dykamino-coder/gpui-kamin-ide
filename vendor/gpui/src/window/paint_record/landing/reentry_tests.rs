//! Where a node's commands come from several records: the node a record
//! paints inside of, whose children's deferred paint the record runs under it
//! again, stays one node; the commands of a node opened inside the record that
//! two of its landings show stay torn.

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

fn quads(window: &Window) -> Vec<Bounds<ScaledPixels>> {
    let quad = |op: &PaintOperation| match op {
        PaintOperation::Primitive(p @ Primitive::Quad(_)) => Some(crate::scene::placed_bounds(p)),
        _ => None,
    };
    window.next_frame.scene.paint_operations.iter().filter_map(quad).collect()
}

fn quad(window: &mut Window, bounds: Bounds<Pixels>) {
    let scale = window.scale_factor();
    let content_mask = window.content_mask().scale(scale);
    window.insert_scene_primitive(Quad { bounds: bounds.scale(scale), content_mask, ..Default::default() });
}

#[crate::test]
fn a_record_that_runs_deferred_paint_under_the_node_it_paints_in_keeps_it_one_node(cx: &mut TestAppContext) {
    cx.add_window(|_, _| Empty)
        .update(cx, |_, window, cx| {
            window.scroll_interest_margin = px(100.);
            let root = ScrollHandle::new();
            root.set_max_offset(point(px(0.), px(10_000.)));
            let id = ScrollNodeId::of(&root);
            window.invalidator.set_phase(DrawPhase::Prepaint);
            let (_, prepaint) = window.record_retained_prepaint(|window| {
                window.with_scroll_node_prepaint(&root, rect(0., 0., 200., 100.), |_| {});
            });
            window.invalidator.set_phase(DrawPhase::Paint);
            let (_, paint) = window.with_paint_pass(|window| {
                window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(id, |window| {
                        quad(window, rect(0., 0., 10., 10.));
                        // A positioned child's paint, deferred and run again under the node inside a record.
                        let node = window.next_frame.scroll_paint.open.last().copied();
                        let (_, record) = window
                            .record_paint(rect(0., 0., 200., 100.), cx, |window, _| {
                                let (mut reentry, mut swapped) = (None, false);
                                window.reenter_scroll_node(node, &mut reentry, &mut swapped);
                                quad(window, rect(20., 20., 10., 10.));
                                window.reenter_scroll_node(node, &mut reentry, &mut swapped);
                            })
                            .unwrap();
                        let at = PaintPlacement { source: point(px(0.), px(0.)), destination: point(px(0.), px(0.)), clip: rect(0., 0., 200., 100.) };
                        window.paint_plan(&PaintPlan::record(record), at).unwrap();
                    });
                })
            });
            let node = window.next_frame.scroll_nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(node.torn, None, "the record's commands under the node tore it");
            std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
            window.next_frame.clear();
            root.set_offset(point(px(0.), px(-15.)));
            let plan = window.plan_scrolled_replay(&prepaint, &paint, None).expect("the node replays scrolled");
            window.invalidator.set_phase(DrawPhase::Prepaint);
            window.replay_retained_prepaint_scrolled(&prepaint, &plan);
            window.invalidator.set_phase(DrawPhase::Paint);
            window.replay_retained_paint_scrolled(&paint, &plan);
            let scale = window.scale_factor();
            assert_eq!(quads(window), vec![rect(20., 5., 10., 10.).scale(scale)], "both commands move with the node");
        })
        .unwrap();
}

#[crate::test]
fn a_node_inside_a_record_shown_by_two_columns_is_torn(cx: &mut TestAppContext) {
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
            let (_, paint) = window.with_paint_pass(|window| {
                window.record_retained_paint(|window| {
                    window.with_scroll_node_paint(outer_id, |window| {
                        let (_, record) = window
                            .record_paint(rect(0., 0., 100., 120.), cx, |window, _| {
                                // Across the column break: both columns show it.
                                window.with_scroll_node_paint(inner_id, |window| quad(window, rect(10., 50., 10., 20.)));
                            })
                            .unwrap();
                        let plan = PaintPlan::record(record);
                        let first = PaintPlacement { source: point(px(0.), px(0.)), destination: point(px(0.), px(0.)), clip: rect(0., 0., 100., 60.) };
                        let second = PaintPlacement { source: point(px(0.), px(60.)), destination: point(px(110.), px(0.)), clip: rect(110., 0., 100., 60.) };
                        window.paint_plan(&plan, first).unwrap();
                        window.paint_plan(&plan, second).unwrap();
                    });
                })
            });
            let node = window.next_frame.scroll_nodes.iter().find(|node| node.id == inner_id).unwrap();
            assert!(node.torn.is_some(), "the same commands shown twice did not tear the node");
            std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
            window.next_frame.clear();
            outer.set_offset(point(px(0.), px(-10.)));
            assert!(window.plan_scrolled_replay(&prepaint, &paint, None).is_none(), "a torn node replayed scrolled");
        })
        .unwrap();
}
