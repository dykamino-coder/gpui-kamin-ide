//! Nested motion and reversed axes retain the same paint at fractional offsets.

use super::*;
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

fn arrows(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| window.painted_paths())
        .into_iter()
        .filter(|path| !path.bounds.intersect(&path.content_mask.bounds).is_empty())
        .map(|mut path| {
            path.id = gpui::PathId(0);
            path.order = 0;
            format!("{path:?}")
        })
        .collect()
}

#[gpui::test]
fn nested_and_reversed_smooth_frames_only_translate(cx: &mut TestAppContext) {
    for (reversed, classic, perspective) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (true, true, false),
        (false, true, true),
    ] {
        let mut content: String = if reversed {
            "<div style='width:3000px;height:80px;background:linear-gradient(90deg,red,blue)'></div>".into()
        } else {
            rows(300)
        };
        if perspective {
            content = format!(
                "<div style='transform:rotateY(20deg);opacity:.5;background:lime'>{content}</div>"
            );
        }
        let theme = if classic {
            "scrollbar-color:#345 #ddd"
        } else {
            ""
        };
        let depth = if perspective { "perspective:500px" } else { "" };
        let axis = if reversed { "direction:rtl" } else { "" };
        let html = format!(
            "{STYLE}<div style='height:200px;width:300px;overflow:scroll;{axis};{theme};{depth}'>{content}</div>{}",
            rows(100)
        );
        let mut app = cx.clone();
        let (view, vcx) = open(&mut app, &html);
        at(vcx, 0);
        let (_guard, _reader) = crate::why::test_reader();
        vcx.simulate_event(ScrollWheelEvent {
            position: point(px(100.), px(40.)),
            delta: ScrollDelta::Lines(if reversed {
                point(-3., 0.)
            } else {
                point(0., -3.)
            }),
            modifiers: Modifiers::default(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        for ms in (16..=224).step_by(16) {
            let before = vcx.update(|_, cx| view.read(cx).document.retained_frames().0);
            at(vcx, ms);
            assert_eq!(
                vcx.update(|_, cx| view.read(cx).document.retained_frames().0),
                before,
                "reversed={reversed}, classic={classic}, perspective={perspective}, vsync {ms}: {:?}",
                crate::why::take()
            );
            if ms % 32 == 0 {
                let replayed = painted(vcx);
                let paths = arrows(vcx);
                let groups = super::super::scroll_replay_tests::groups::groups(vcx);
                let maps = vcx.update(|w, _| w.painted_plane_maps());
                let size = vcx.update(|window, _| window.viewport_size());
                for size in [gpui::size(size.width, size.height + px(1.)), size] {
                    vcx.simulate_resize(size);
                    at(vcx, ms);
                }
                if let Err(diff) = same_paint(&replayed, &painted(vcx)) {
                    panic!(
                        "reversed={reversed}, classic={classic}, perspective={perspective}, vsync {ms}: {diff}"
                    );
                }
                assert_eq!(paths, arrows(vcx), "arrow states at {ms}");
                assert_eq!(
                    groups,
                    super::super::scroll_replay_tests::groups::groups(vcx),
                    "nested groups at {ms}"
                );
                assert_eq!(
                    maps,
                    vcx.update(|w, _| w.painted_plane_maps()),
                    "nested maps at {ms}"
                );
            }
        }
        let journal = crate::why::take().journal;
        assert!(journal.scroll_animation_frames > 1);
        assert_eq!(journal.scroll_animation_builds, 0);
    }
}
