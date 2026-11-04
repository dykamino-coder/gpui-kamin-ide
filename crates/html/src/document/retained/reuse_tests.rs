//! Пропуск блоков корневого потока вне экрана после правки (`layout::reuse`).
//! Пропускаются блоки, отстоящие от экрана дальше его размера: документы
//! длиннее нескольких экранов. Первая сборка документа ничего не пишет:
//! записи делает вторая сборка с тем же ключом потока, пропускает третья.

use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

fn rows(edit: usize, text: &str) -> String {
    let rows: String = (0..600)
        .map(|i| {
            let body = if i == edit {
                text.to_string()
            } else {
                format!("row {i}")
            };
            format!("<div style='height:20px'>{body}</div>")
        })
        .collect();
    format!("<body style='margin:0'>{rows}</body>")
}

pub(super) fn reused(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> (u64, u64, u64) {
    cx.update(|_, cx| view.read(cx).doc.reused_blocks())
}

pub(super) fn update(view: &gpui::Entity<View>, cx: &mut VisualTestContext, html: &str) {
    cx.update(|_, cx| view.update(cx, |v, _| v.doc.update(html, "").unwrap()));
    frame(cx);
}

/// Open `first`, then rebuild with `second`: the second build records.
pub(super) fn settled<'a>(
    cx: &'a mut TestAppContext,
    first: &str,
    second: &str,
) -> (gpui::Entity<View>, &'a mut VisualTestContext) {
    let (view, cx) = open(cx, first);
    frame(cx);
    assert_eq!(
        reused(&view, cx),
        (0, 0, 0),
        "the first build of a document records nothing"
    );
    update(&view, cx, second);
    (view, cx)
}

#[gpui::test]
fn an_offscreen_edit_skips_the_unchanged_offscreen_blocks(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let (recorded, skipped, rebuilt) = reused(&view, cx);
    assert!(
        recorded > 400,
        "settled blocks were not recorded: {recorded}"
    );
    assert_eq!((skipped, rebuilt), (0, 0));
    update(&view, cx, &rows(300, "row 300"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(skipped > 300, "offscreen blocks were rebuilt: {skipped}");
    assert_eq!(rebuilt, 0, "no skipped block reached the screen");
}

#[gpui::test]
fn a_skipped_block_that_reaches_the_screen_rebuilds_the_frame(cx: &mut TestAppContext) {
    let tall = |end: &str| format!("{}{end}", "x ".repeat(6000));
    let page = |text: &str| rows(5, text).replace("<div style='height:20px'>x", "<div>x");
    let (view, cx) = settled(cx, &page(&tall("x")), &page(&tall("y")));
    update(&view, cx, &page("x"));
    frame(cx);
    let (_, _, rebuilt) = reused(&view, cx);
    assert_eq!(rebuilt, 1, "blocks pulled into view were left skipped");
}

#[gpui::test]
fn blocks_below_a_grown_block_are_skipped_at_their_new_place(cx: &mut TestAppContext) {
    let grown = format!("{}x", "x ".repeat(300));
    let page = |text: &str| rows(5, text).replace("<div style='height:20px'>x", "<div>x");
    let (view, cx) = settled(cx, &page("x"), &page("xy"));
    // The onscreen rows below the grown one move: their on-screen replay
    // (`layout::reuse::replay`) fails the frame check once and the frame is
    // built without replays and skips; they are built from then on while the
    // flow key stays. The second settled build records them for replay.
    update(&view, cx, &page("x"));
    update(&view, cx, &page(&grown));
    assert_eq!(reused(&view, cx).2, 1);
    update(&view, cx, &page("x"));
    update(&view, cx, &page(&grown));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 300,
        "shifted offscreen blocks were rebuilt: {skipped}"
    );
    assert_eq!(rebuilt, 1);
}

#[gpui::test]
fn blocks_after_a_float_are_always_built(cx: &mut TestAppContext) {
    let float = "<div style='height:20px'><span style='float:left;height:9000px'>f</span></div>";
    let page =
        |text: &str| rows(300, text).replacen("<div style='height:20px'>row 2</div>", float, 1);
    let (view, cx) = settled(cx, &page("row 300"), &page("row edited"));
    update(&view, cx, &page("row 300"));
    let (_, skipped, _) = reused(&view, cx);
    assert_eq!(skipped, 0, "a float may reach into the blocks after it");
}

#[gpui::test]
fn a_flow_resized_every_frame_records_nothing_and_drops_its_records(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let (recorded, _, _) = reused(&view, cx);
    for width in [401., 400.] {
        cx.update(|_, cx| view.update(cx, |v, _| v.viewport.0 = width));
        frame(cx);
    }
    let (after, skipped, rebuilt) = reused(&view, cx);
    assert_eq!(after, recorded, "records of a changing flow never pay off");
    // Back at the recorded width: records of another key were dropped (they
    // would hold the styles of their frame), so nothing is skipped.
    assert_eq!((skipped, rebuilt), (0, 0));
    update(&view, cx, &rows(300, "row 300"));
    let (again, skipped, _) = reused(&view, cx);
    assert!(again > recorded + 400 && skipped == 0, "{again} {skipped}");
    update(&view, cx, &rows(300, "row edited"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(skipped > 300 && rebuilt == 0, "{skipped} {rebuilt}");
}

#[gpui::test]
fn blocks_near_the_screen_are_not_recorded(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let (recorded, _, _) = reused(&view, cx);
    // 600 rows of 20px in a 1920x1080 test window: the 150 rows within one
    // screen size of it (y < 3000) could never be skipped by their record.
    assert!(
        (400..=600 - 2 - 140).contains(&recorded),
        "near blocks were recorded: {recorded}"
    );
}

/// A standards document without box styles on its wrappers loses both
/// (`document::embedded::unwrap_document`): its top-level blocks are the root
/// flow, laid out by a block container. With a padded `body` only `html` goes.
#[gpui::test]
fn blocks_of_unwrapped_documents_are_skipped(cx: &mut TestAppContext) {
    for body in ["", " style='padding:3px'"] {
        unwrapped(cx, body);
    }
}

fn unwrapped(cx: &mut TestAppContext, body: &str) {
    let page = |edit: &str| -> String {
        let paragraphs: String = (0..600)
            .map(|i| {
                let word = if i == 300 { edit } else { "edit" };
                format!("<p>שלום עולם {word} {i} مرحبا <b>מודגש</b> text.</p>\n")
            })
            .collect();
        format!(
            "<!doctype html><html dir=rtl><head><style>p{{margin:2px 0}}</style></head><body{body}>\n{paragraphs}</body></html>"
        )
    };
    let (view, cx) = settled(cx, &page("edit"), &page("odit"));
    update(&view, cx, &page("edit"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 300,
        "unwrapped root flow was rebuilt: {body} {skipped}"
    );
    assert_eq!(rebuilt, 0);
}

#[gpui::test]
fn right_to_left_blocks_are_skipped_too(cx: &mut TestAppContext) {
    let page = |text: &str| format!("<html dir=rtl>{}</html>", rows(300, text));
    let (view, cx) = settled(cx, &page("row 300"), &page("row edited"));
    update(&view, cx, &page("row 300"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 300,
        "right-to-left blocks were rebuilt: {skipped}"
    );
    assert_eq!(rebuilt, 0);
}
