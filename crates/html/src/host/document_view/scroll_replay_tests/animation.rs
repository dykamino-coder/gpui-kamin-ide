//! Prepare distant motion before moving; every displayed step replays the record.
use super::*;
use crate::document::ScrollBehavior;
use gpui::AnimationElapsedTime;
use std::time::Duration;

fn at(cx: &mut VisualTestContext, ms: u64) {
    cx.update(|window, cx| {
        cx.set_global(AnimationElapsedTime(Duration::from_millis(ms)));
        window.simulate_next_frame(cx);
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

#[gpui::test]
fn far_smooth_path_builds_before_motion_then_only_translates(cx: &mut TestAppContext) {
    let html = format!(
        "<style>html{{background:#fafafa}}</style>{}",
        rows(&fixed_box())
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    at(vcx, 0);
    let (_guard, _reader) = crate::why::test_reader();
    let before = frames(&view, vcx);
    let inside = vcx.update(|_, cx| view.read(cx).document.builds_inside_draw());
    vcx.update(|window, cx| {
        view.read(cx)
            .document
            .scroll_to(0., 6500., ScrollBehavior::Smooth, window, cx);
    });
    at(vcx, 0);
    assert_eq!(root(&view, vcx).offset().y, px(0.));
    assert_eq!(frames(&view, vcx).0, before.0 + 1);
    assert_eq!(
        vcx.update(|_, cx| view.read(cx).document.builds_inside_draw()),
        inside
    );
    let prepared = frames(&view, vcx);
    let preparation = crate::why::take();
    assert_eq!(preparation.journal.scroll_animation_frames, 0);
    for ms in (16..=560).step_by(16) {
        at(vcx, ms);
        if frames(&view, vcx).0 != prepared.0 {
            panic!("vsync {ms}: {:?}", crate::why::take());
        }
        if [96, 256, 544].contains(&ms) {
            let offset = root(&view, vcx).offset();
            let range = root(&view, vcx).max_offset();
            let mut app = cx.clone();
            let (fresh_view, fresh) = open(&mut app, &html);
            root(&fresh_view, fresh).set_offset(offset);
            frame(fresh);
            assert_eq!(
                range,
                root(&fresh_view, fresh).max_offset(),
                "range vsync {ms}"
            );
            assert_same(
                &format!("vsync {ms}"),
                &painted(vcx),
                &fresh_quads(cx, &html, offset),
            );
        }
    }
    assert_eq!(root(&view, vcx).offset().y, px(-6500.));
    let entry = crate::why::take();
    assert!(entry.journal.scroll_animation_frames > 1);
    assert_eq!(entry.journal.scroll_animation_builds, 0);
}
