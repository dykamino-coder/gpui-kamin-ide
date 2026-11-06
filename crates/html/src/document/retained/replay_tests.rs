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
    // The first settled build builds every block: it records the paragraphs
    // only for skipping, and the next build records the ones it still builds.
    update(&view, cx, &page("one"));
    assert_eq!(replayed(&view, cx), 1, "the table was rebuilt");
    update(&view, cx, &page("two"));
    assert_eq!(
        replayed(&view, cx),
        3,
        "the table and the tail were rebuilt"
    );
    // A taller paragraph moves the tail: its replay fails the frame check and
    // the frame is built again; the tail is then built until the key changes.
    update(&view, cx, &page(&"word ".repeat(400)));
    let (_, _, rebuilt) = reused(&view, cx);
    assert_eq!(rebuilt, 1, "a moved block was replayed at its old place");
}

/// While the width changes, a block that fills it (`width: auto`) gets no
/// replay record: it could not replay at another width. Nor does the table,
/// whose last record read the width. The first build at the settled width
/// builds the whole flow: it records the table for replay and the tail
/// paragraph only for skipping. The second records the tail too, and both
/// replay from the third.
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
    let mut last = replayed(&view, cx);
    let mut steps = Vec::new();
    for text in ["one", "two", "one"] {
        update(&view, cx, &page(text));
        let now = replayed(&view, cx);
        steps.push(now - last);
        last = now;
    }
    assert_eq!(
        steps,
        [0, 1, 2],
        "replays per build after the resize: the table from the second, the tail from the third"
    );
}

/// A block whose last replay record read the width from outside (a table of
/// a percentage width) is not recorded again while the width keeps changing:
/// such a record fits only the same widths. Once the width settles it is
/// recorded by the first build and replays from the next one.
#[gpui::test]
fn blocks_that_read_the_width_are_not_recorded_while_it_changes(cx: &mut TestAppContext) {
    let page = |text: &str| {
        format!(
            "<body style='margin:0'><table style='width:50%'><tr><td>a</td></tr></table>\
             <p>{text}</p><p>tail</p></body>"
        )
    };
    let (view, cx) = settled(cx, &page("one"), &page("two"));
    let mut last = replayed(&view, cx);
    let mut steps = Vec::new();
    for width in [401., 402., 402., 402.] {
        cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
        let text = if steps.len() % 2 == 0 { "one" } else { "two" };
        update(&view, cx, &page(text));
        let now = replayed(&view, cx);
        steps.push(now - last);
        last = now;
    }
    assert_eq!(
        steps,
        [0, 0, 0, 1],
        "replays per build: the table is recorded only once the width settles"
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
    let (view, cx) = settled(cx, &page("two"), &page("one"));
    // The second settled build records the paragraphs for replay.
    update(&view, cx, &page("two"));
    update(&view, cx, &page("one"));
    assert_eq!(replayed(&view, cx), 2, "head and tail were rebuilt");
    let replay = scene(cx);
    let (_, fresh) = open(cx, &page("one"));
    frame(fresh);
    assert_eq!(replay, scene(fresh), "the replayed frame differs");
}

/// Recording visits the deep child through the same continuation as a fresh
/// build; subsequent edits claim its layout whole and replay its line jobs.
#[test]
fn deep_recording_and_replay_keep_the_line_phase_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let mut app = TestAppContext::single();
            let head = format!(
                "{}<span style='background:lime;text-decoration:underline'>leaf</span>{}",
                "<div>".repeat(5000),
                "</div>".repeat(5000),
            );
            let page = |text: &str| {
                format!("<body style='margin:0'>{head}<p>{text}</p><p>tail</p></body>")
            };
            let (view, cx) = settled(&mut app, &page("two"), &page("one"));
            update(&view, cx, &page("two"));
            update(&view, cx, &page("one"));
            assert_eq!(replayed(&view, cx), 2, "the deep block or tail was rebuilt");
            let replay = scene(cx);
            let (_, fresh) = open(cx, &page("one"));
            frame(fresh);
            assert_eq!(replay, scene(fresh), "deep replay changed the line phase");
        })
        .unwrap()
        .join()
        .unwrap();
}
