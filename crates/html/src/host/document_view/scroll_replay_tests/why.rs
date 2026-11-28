//! Depth planes replay through motion, including paint entering the visible clip.

use super::*;
use crate::document::ScrollBehavior;
use gpui::AnimationElapsedTime;
use std::time::Duration;

#[gpui::test]
fn depth_planes_scroll_without_building_and_match_fresh(cx: &mut TestAppContext) {
    let html = rows(
        "<div style='perspective:100px'><div style='transform:rotateY(20deg);opacity:.5'>x</div></div><div style='height:350px'></div><div style='perspective:100px'><div style='transform:rotateY(-15deg);background:lime'>entering</div></div>",
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    vcx.update(|_, cx| cx.set_global(AnimationElapsedTime(Duration::ZERO)));
    let (_one, _sub) = crate::why::test_reader();
    let (built, _) = frames(&view, vcx);
    vcx.update(|window, cx| {
        view.read(cx)
            .document
            .scroll_to(0., 120., ScrollBehavior::Smooth, window, cx);
    });
    for ms in (16..=500).step_by(16) {
        vcx.update(|window, cx| {
            cx.set_global(AnimationElapsedTime(Duration::from_millis(ms)));
            window.simulate_next_frame(cx);
            window.refresh();
            window.draw(cx).clear(cx);
        });
        assert_eq!(
            frames(&view, vcx).0,
            built,
            "vsync {ms}: {:?}",
            crate::why::take()
        );
        if [32, 96, 496].contains(&ms) {
            let offset = root(&view, vcx).offset();
            let (fresh, groups, maps) = fresh_with(cx, &html, offset, |c| {
                (
                    painted(c),
                    super::groups::groups(c),
                    c.update(|w, _| w.painted_plane_maps()),
                )
            });
            assert_eq!(super::groups::groups(vcx), groups, "3D groups at {ms}");
            assert_eq!(
                vcx.update(|w, _| w.painted_plane_maps()),
                maps,
                "3D maps at {ms}"
            );
            assert_same(&format!("3D vsync {ms}"), &painted(vcx), &fresh);
        }
    }
    let journal = crate::why::take().journal;
    assert!(journal.scroll_animation_frames > 1);
    assert_eq!(journal.scroll_animation_builds, 0);
}
