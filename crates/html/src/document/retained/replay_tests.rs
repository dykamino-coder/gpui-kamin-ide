//! Повтор блоков корневого потока на экране (`layout::reuse::replay`): блок,
//! чьё поддерево и место не изменились, не собирается, а кадр с повтором
//! рисует те же команды, что свежая сборка, вместе с заданиями фазы строк.

use super::reuse_tests::{reused, settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_blocks())
}

/// Quads and underlines of the shown frame in paint order.
fn scene(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter().map(|q| format!("{q:?}"));
        let lines = window
            .painted_underlines()
            .into_iter()
            .map(|u| format!("{u:?}"));
        quads.chain(lines).collect()
    })
}

#[gpui::test]
fn unchanged_onscreen_blocks_replay_and_a_moved_one_is_rebuilt(cx: &mut TestAppContext) {
    let page = |text: &str| {
        format!(
            "<body style='margin:0'><table style='width:300px'><tr><td>a</td><td>b</td></tr></table>\
             <p>{text}</p><p>tail</p></body>"
        )
    };
    let (view, cx) = settled(cx, &page("one"), &page("two"));
    assert_eq!(
        replayed(&view, cx),
        0,
        "the recording build replays nothing"
    );
    update(&view, cx, &page("one"));
    assert_eq!(
        replayed(&view, cx),
        2,
        "the table and the tail were rebuilt"
    );
    // A taller paragraph moves the tail: its replay fails the frame check and
    // the frame is built again; the tail is then built until the key changes.
    update(&view, cx, &page(&"word ".repeat(400)));
    let (_, _, rebuilt) = reused(&view, cx);
    assert_eq!(rebuilt, 1, "a moved block was replayed at its old place");
}

/// While the width changes, a block that fills it (`width: auto`) gets no
/// replay record: it could not replay at another width. The tail paragraph is
/// recorded by the first build at the settled width and replays from the
/// next one; the definite-width table is recorded either way.
#[gpui::test]
fn width_filling_blocks_are_recorded_for_replay_only_once_the_width_settles(
    cx: &mut TestAppContext,
) {
    let page = |text: &str| {
        format!(
            "<body style='margin:0'><table style='width:300px'><tr><td>a</td></tr></table>\
             <p>{text}</p><p>tail</p></body>"
        )
    };
    let (view, cx) = settled(cx, &page("one"), &page("two"));
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = 401.));
    frame(cx);
    let resized = replayed(&view, cx);
    update(&view, cx, &page("one"));
    let first = replayed(&view, cx);
    update(&view, cx, &page("two"));
    let settled_width = replayed(&view, cx);
    assert_eq!(
        (settled_width - first) - (first - resized),
        1,
        "the tail must replay only after a build at the settled width"
    );
}

/// Line content (CSS 2.1 Appendix E step 7) of a block paints when its
/// stacking context drains, after the backgrounds of later blocks: a replay
/// puts it back into the same place of that phase.
#[gpui::test]
fn a_replayed_frame_paints_the_line_phase_of_a_fresh_build(cx: &mut TestAppContext) {
    let page = |text: &str| {
        format!(
            "<body style='margin:0'>\
             <p><span style='background:lime;text-decoration:underline'>head</span> h</p>\
             <p>{text}</p>\
             <p style='background:silver;margin-top:-30px'>\
             <span style='background:red'>tail</span> <u>u</u></p></body>"
        )
    };
    let (view, cx) = settled(cx, &page("one"), &page("two"));
    update(&view, cx, &page("one"));
    assert_eq!(replayed(&view, cx), 2, "head and tail were rebuilt");
    let replay = scene(cx);
    let (_, fresh) = open(cx, &page("one"));
    frame(fresh);
    assert_eq!(replay, scene(fresh), "the replayed frame differs");
}
