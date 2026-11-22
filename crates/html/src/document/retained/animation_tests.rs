//! Document animation input changes layout while a stopped frame can replay.

use super::{
    box_repaint_tests::{frames, quads},
    tests::{frame, open_document},
};
use crate::{AnimationTimeSource, Document};
use gpui::{Modifiers, TestAppContext, point, px};
use std::time::Duration;

const PAGE: &str = "<style>body{margin:0}@keyframes k{from{width:20px}to{width:100px}}\
    .k{height:20px;background:red;animation:k 1s linear infinite}</style>\
    <div class=k></div><div class=k></div>";

fn document(html: &str, ms: u64) -> Document {
    let doc = Document::builder(html, "").build().unwrap();
    doc.set_animation_elapsed(Some(Duration::from_millis(ms)));
    doc
}

fn fresh(cx: &mut TestAppContext, ms: u64) -> Vec<String> {
    let (_, cx) = open_document(cx, document(PAGE, ms));
    frame(cx);
    frame(cx);
    quads(cx)
}

#[gpui::test]
fn snapshot_seek_matches_fresh_and_equal_time_replays(cx: &mut TestAppContext) {
    let expected_250 = fresh(cx, 250);
    let expected_750 = fresh(cx, 750);
    assert_ne!(expected_250, expected_750, "fixture must visibly animate");
    let (view, cx) = open_document(cx, document(PAGE, 250));
    frame(cx);
    frame(cx);
    assert_eq!(quads(cx), expected_250);
    let before = frames(&view, cx);
    frame(cx);
    assert_eq!(frames(&view, cx), (before.0, before.1 + 1));
    for (ms, expected) in [(750, expected_750), (250, expected_250)] {
        cx.update(|_, app| {
            view.read(app)
                .doc
                .set_animation_elapsed(Some(Duration::from_millis(ms)));
        });
        frame(cx);
        assert_eq!(quads(cx), expected);
        let journal = cx.update(|_, app| view.read(app).doc.animation_frame().unwrap());
        assert_eq!(journal.source, AnimationTimeSource::DocumentSnapshot);
        assert_eq!(journal.elapsed, Duration::from_millis(ms));
        assert!(journal.keyframe_samples >= 2);
        assert!(!journal.keyframe_requested && !journal.transition_requested);
        let repeated = cx.update(|_, app| view.read(app).doc.animation_frame().unwrap());
        assert_eq!(journal, repeated, "reading the journal changed it");
    }
}

#[gpui::test]
fn static_replay_keeps_a_current_zero_work_journal(cx: &mut TestAppContext) {
    let (view, cx) = open_document(
        cx,
        document("<div style='width:20px;height:20px'></div>", 0),
    );
    frame(cx);
    frame(cx);
    let before = frames(&view, cx);
    let previous = cx.update(|_, app| view.read(app).doc.animation_frame().unwrap());
    cx.update(|_, app| {
        view.read(app)
            .doc
            .set_animation_elapsed(Some(Duration::from_millis(17)))
    });
    frame(cx);
    let current = cx.update(|_, app| view.read(app).doc.animation_frame().unwrap());
    assert_eq!(frames(&view, cx), (before.0, before.1 + 1));
    assert!(current.frame_serial > previous.frame_serial);
    assert_eq!(current.elapsed, Duration::from_millis(17));
    assert_eq!(current.keyframe_samples, 0);
    assert_eq!(current.transition_samples, 0);
    assert!(!current.keyframe_requested && !current.transition_requested);
}

#[gpui::test]
fn stopped_hover_transition_changes_once_then_holds_the_sample(cx: &mut TestAppContext) {
    let html = "<style>body{margin:0}.b{height:20px;width:20px;background:red;transition:1s}\
        .b:hover{width:100px}</style><div class=b></div>";
    let (view, cx) = open_document(cx, document(html, 0));
    frame(cx);
    cx.simulate_mouse_move(point(px(5.), px(5.)), None, Modifiers::default());
    frame(cx);
    frame(cx);
    let initial = quads(cx);
    cx.update(|_, app| {
        view.read(app)
            .doc
            .set_animation_elapsed(Some(Duration::from_millis(500)));
    });
    frame(cx);
    let halfway = quads(cx);
    assert_ne!(initial, halfway, "hover fixture did not advance");
    let before = frames(&view, cx);
    frame(cx);
    assert_eq!(quads(cx), halfway);
    assert_eq!(frames(&view, cx), (before.0, before.1 + 1));
    let journal = cx.update(|_, app| view.read(app).doc.animation_frame().unwrap());
    assert!(!journal.keyframe_requested && !journal.transition_requested);
}
