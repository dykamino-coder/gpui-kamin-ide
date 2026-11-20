//! A wheel step of a nested scroller, or of the root under a sticky box,
//! inside the recorded interest rect replays the frame moved: no build, and
//! the painted quads are those of the same document rebuilt at that offset.

use super::paint_compare::{painted, same_paint};
use super::tests::{frame, open};
use super::*;
use gpui::{Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point};

fn wheel_at(cx: &mut VisualTestContext, x: f32, y: f32, dy: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
}

fn rows(n: usize) -> String {
    (0..n)
        .map(|i| format!("<div class=r>row {i}</div>"))
        .collect()
}

const STYLE: &str =
    "<style>body{margin:0}.r{height:20px}.r:nth-child(odd){background:#eee}</style>";

/// Steps of `dy` at `(x, y)`; per step the builds and replays it cost, after
/// checking the frame against the same document rebuilt at that offset.
fn steps(cx: &mut TestAppContext, html: &str, at: (f32, f32), dys: &[f32]) -> Vec<(u64, u64)> {
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, html);
    frame(vcx);
    let frames = |vcx: &mut VisualTestContext| {
        vcx.update(|_, cx| view.read(cx).document.retained_frames())
    };
    let mut costs = Vec::new();
    for (i, &dy) in dys.iter().enumerate() {
        let (b0, r0) = frames(vcx);
        wheel_at(vcx, at.0, at.1, dy);
        frame(vcx);
        let (b1, r1) = frames(vcx);
        let after = painted(vcx);
        // The same document rebuilt: element states (scroll offsets) stay.
        vcx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.document
                    .update(&format!("{html}<!-- fresh {i} -->"), "")
                    .unwrap();
                cx.notify();
            })
        });
        frame(vcx);
        frame(vcx);
        let fresh = painted(vcx);
        if let Err(diff) = same_paint(&after, &fresh) {
            panic!("step {i} by {dy} (builds {}, replays {}): {diff}", b1 - b0, r1 - r0);
        }
        costs.push((b1 - b0, r1 - r0));
    }
    costs
}

#[gpui::test]
fn a_nested_scroller_steps_without_a_build(cx: &mut TestAppContext) {
    let html = format!(
        "{STYLE}<div style='height:200px;width:300px;overflow:auto'>{}</div><main>{}</main>",
        rows(300),
        rows(100)
    );
    let costs = steps(cx, &html, (100., 100.), &[-120., -120., -120., 120.]);
    assert!(costs.iter().all(|c| c.0 == 0), "{costs:?}");
}

#[gpui::test]
fn a_root_step_under_a_sticky_box_replays(cx: &mut TestAppContext) {
    let html = format!(
        "{STYLE}<div style='position:sticky;top:0;height:30px;background:blue'></div><main>{}</main>",
        rows(400)
    );
    let costs = steps(cx, &html, (200., 150.), &[-120., -120., -120., 120.]);
    assert!(costs.iter().all(|c| c.0 == 0), "{costs:?}");
}
