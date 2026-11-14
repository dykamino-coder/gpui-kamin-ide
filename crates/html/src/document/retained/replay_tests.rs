//! Повтор блоков корневого потока на экране (`layout::reuse::replay`): блок,
//! чьё поддерево и место не изменились, не собирается, а кадр с повтором
//! рисует те же команды, что свежая сборка, вместе с заданиями фазы строк.
//! Тело страниц — `display: flow-root`: записи пропуска (`recorder`) его
//! детям не остаются, и повтор здесь — только повтор с поддеревом раскладки.
//! Первая устоявшаяся сборка всё равно оборачивает их для пропуска, поэтому
//! для повтора их пишет лишь следующая (`mark::replay_worth_recording`).

use super::reuse_tests::{reused, settled, update};
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// Blocks replayed with their layout subtree claimed whole.
fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| {
        let doc = &view.read(cx).doc;
        assert_eq!(
            doc.replayed_blocks(),
            doc.replayed_subtrees(),
            "a child of a flow-root body was replayed from a skip record"
        );
        doc.replayed_subtrees()
    })
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
            "<body style='margin:0;display:flow-root'><table style='width:300px'><tr><td>a</td><td>b</td></tr></table>\
             <p>{text}</p><p>tail</p></body>"
        )
    };
    let (view, cx) = settled(cx, &page("one"), &page("two"));
    assert_eq!(
        replayed(&view, cx),
        0,
        "the recording build replays nothing"
    );
    // The first settled build wraps every block only for skipping, and the
    // next build records the ones it still builds for replay.
    update(&view, cx, &page("one"));
    assert_eq!(replayed(&view, cx), 0, "a block replayed a skip wrapper");
    update(&view, cx, &page("two"));
    assert_eq!(
        replayed(&view, cx),
        2,
        "the table and the tail were rebuilt"
    );
    assert_eq!(reused(&view, cx).2, 0, "an unmoved replay was refused");
    // A taller paragraph moves the tail: a block with text is not moved
    // (`ReplaySlot::valid`), its replay fails the frame check and the frame
    // is built again.
    update(&view, cx, &page(&"word ".repeat(400)));
    let (_, _, rebuilt) = reused(&view, cx);
    assert_eq!(rebuilt, 1, "a moved block was replayed at its old place");
}

/// A block without text moved by whole device pixels with all its ink on
/// screen replays at the new place (`ReplaySlot::valid`): its paint is the
/// recorded one moved, and the frame is the fresh build's.
#[gpui::test]
fn a_block_moved_by_whole_pixels_replays_at_its_new_place(cx: &mut TestAppContext) {
    let page = |text: &str| {
        format!(
            "<body style='margin:0;display:flow-root'>\
             <p style='margin:0;line-height:20px;width:40px'>{text}</p>\
             <div style='height:20px;background:silver'></div>\
             <div style='height:30px;background:gray'>\
             <div style='width:30px;height:10px;background:lime'></div></div></body>"
        )
    };
    let (view, cx) = settled(cx, &page("one"), &page("two"));
    // The second settled build records the paragraphs for replay.
    update(&view, cx, &page("one"));
    update(&view, cx, &page("two"));
    let before = replayed(&view, cx);
    // A second line of the narrow head moves the rest by 20px.
    let taller = page("two two");
    update(&view, cx, &taller);
    assert_eq!(
        (replayed(&view, cx) as i64 - before as i64, reused(&view, cx).2),
        (2, 0),
        "the moved boxes were not replayed"
    );
    let replay = scene(cx);
    let (_, fresh) = open(cx, &taller);
    frame(fresh);
    assert_eq!(replay, scene(fresh), "the moved frame differs");
}

/// While the width changes, a block that fills it (`width: auto`) gets no
/// replay record: it could not replay at another width. The table of a fixed
/// width, never recorded for replay before, is recorded by the resized build
/// and replays at that width from the next one. The first build at the
/// settled width wraps the tail paragraph only for skipping; the second
/// records it for replay, and it replays from the third.
#[gpui::test]
fn width_filling_blocks_are_recorded_for_replay_only_once_the_width_settles(
    cx: &mut TestAppContext,
) {
    let page = |text: &str| {
        format!(
            "<body style='margin:0;display:flow-root'><table style='width:300px'><tr><td>a</td></tr></table>\
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
        [1, 1, 2],
        "replays per build after the resize: the table from the first, the tail from the third"
    );
}

/// A block whose last replay record read the width from outside (a table of
/// a percentage width) is not recorded again while the width keeps changing:
/// such a record fits only the same widths. Once the width settles, the
/// first build wraps it (and the width-filling tail) only for skipping, the
/// second records both for replay, and they replay from the third.
#[gpui::test]
fn blocks_that_read_the_width_are_not_recorded_while_it_changes(cx: &mut TestAppContext) {
    let page = |text: &str| {
        format!(
            "<body style='margin:0;display:flow-root'><table style='width:50%'><tr><td>a</td></tr></table>\
             <p>{text}</p><p>tail</p></body>"
        )
    };
    let (view, cx) = settled(cx, &page("one"), &page("two"));
    let mut last = replayed(&view, cx);
    let mut steps = Vec::new();
    for width in [401., 402., 402., 402., 402.] {
        cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
        let text = if steps.len() % 2 == 0 { "one" } else { "two" };
        update(&view, cx, &page(text));
        let now = replayed(&view, cx);
        steps.push(now - last);
        last = now;
    }
    assert_eq!(
        steps,
        [0, 0, 0, 0, 2],
        "replays per build: the table and the tail are recorded only once the width settles"
    );
}

/// Line content (CSS 2.1 Appendix E step 7) of a block paints when its
/// stacking context drains, after the backgrounds of later blocks: a replay
/// puts it back into the same place of that phase.
#[gpui::test]
fn a_replayed_frame_paints_the_line_phase_of_a_fresh_build(cx: &mut TestAppContext) {
    let page = |text: &str| {
        format!(
            "<body style='margin:0;display:flow-root'>\
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
