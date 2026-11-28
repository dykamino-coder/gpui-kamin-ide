//! An empty fixed anchor belongs to the call site even without scene commands.

use super::super::scroll_tests::{Empty, rect};
use super::*;
use crate::{ScrollHandle, ScrollNodeId, TestAppContext};

#[crate::test]
fn an_empty_anchor_lands_through_nested_records_and_replays(cx: &mut TestAppContext) {
    cx.add_window(|_, _| Empty).update(cx, |_, window, cx| {
        window.scroll_interest_margin = px(100.);
        let outer = ScrollHandle::new();
        outer.set_max_offset(point(px(0.), px(1000.)));
        let id = ScrollNodeId::of(&outer);
        let token = Rc::new(());
        let anchor = ScrollNodeId::of_token(&token);
        let offset = outer.clone();
        window.invalidator.set_phase(DrawPhase::Prepaint);
        let (_, mut prepaint) = window.record_retained_prepaint(|window| {
            window.with_scroll_node_prepaint(&outer, rect(0., 0., 100., 50.), |window| {
                window.with_scroll_anchor_prepaint(anchor, Rc::new(move || -offset.offset()), |_| {});
            });
        });
        window.invalidator.set_phase(DrawPhase::Paint);
        let (_, mut paint) = window.with_paint_pass(|window| window.record_retained_paint(|window| {
            window.with_scroll_node_paint(id, |window| {
                // Empty ranges must land after this command, not at scene index zero.
                window.paint_quad(crate::fill(rect(0., 0., 10., 10.), crate::red()));
                let clip = rect(0., 0., 100., 50.);
                let (_, record) = window.record_paint(clip, cx, |window, cx| {
                    let (_, inner) = window.record_paint(clip, cx, |window, _| {
                        window.with_scroll_node_paint(anchor, |_| {});
                    }).unwrap();
                    window.paint_plan(&PaintPlan::record(inner), PaintPlacement {
                        source: Point::default(), destination: Point::default(), clip,
                    }).unwrap();
                }).unwrap();
                window.paint_plan(&PaintPlan::record(record), PaintPlacement {
                    source: Point::default(), destination: Point::default(), clip,
                }).unwrap();
            });
        }));
        let node = window.next_frame.scroll_nodes.iter().find(|n| n.id == anchor).unwrap();
        assert_eq!(node.painted_in, Some(window.next_frame.scene.scroll.space));
        assert_eq!(node.paint, Some(1..1));
        assert!(node.torn.is_none());
        for y in [10., 20., 40.] {
            std::mem::swap(&mut window.rendered_frame, &mut window.next_frame);
            window.next_frame.clear();
            outer.set_offset(point(px(0.), px(-y)));
            let plan = window.plan_scrolled_replay(&prepaint, &paint, None).expect("empty anchor remains owned by the drawn scene");
            window.invalidator.set_phase(DrawPhase::Prepaint);
            prepaint = window.replay_retained_prepaint_scrolled(&prepaint, &plan);
            window.invalidator.set_phase(DrawPhase::Paint);
            paint = window.replay_retained_paint_scrolled(&paint, &plan);
        }
    }).unwrap();
}
