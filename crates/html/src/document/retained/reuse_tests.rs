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
    // The grown block moves the visible blocks below it: their replay is
    // refused, and one more build of the same frame builds just them.
    update(&view, cx, &page("x"));
    update(&view, cx, &page(&grown));
    assert_eq!(reused(&view, cx).2, 1, "moved visible blocks were replayed");
    // An update adopts a new tree, and its frame counters start anew.
    let builds = cx.update(|_, cx| view.read(cx).doc.retained_frames().0);
    assert_eq!(builds, 2, "the refused frame was built more than twice");
    // The refused blocks are built from then on while the flow key stays
    // (`Store::banned`): moving them again builds no frame twice.
    update(&view, cx, &page("x"));
    update(&view, cx, &page(&grown));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped > 300,
        "shifted offscreen blocks were rebuilt: {skipped}"
    );
    assert_eq!(rebuilt, 1, "a refused block was replayed again");
}

#[gpui::test]
fn blocks_after_a_float_are_always_built(cx: &mut TestAppContext) {
    let float = "<div style='height:20px'><span style='float:left;height:9000px'>f</span></div>";
    let page =
        |text: &str| rows(300, text).replacen("<div style='height:20px'>row 2</div>", float, 1);
    let (view, cx) = settled(cx, &page("row 300"), &page("row edited"));
    update(&view, cx, &page("row 300"));
    let (_, skipped, _) = reused(&view, cx);
    // Only the two rows before the float are skipped (replayed on screen).
    assert!(
        skipped <= 2,
        "a float may reach into the blocks after it: {skipped}"
    );
}

#[gpui::test]
fn a_flow_resized_every_frame_records_nothing_and_drops_its_records(cx: &mut TestAppContext) {
    // Centred rows depend on the flow width (one-line rows at the start
    // edge would not: `one_line_text_blocks_are_reused_across_a_width_change`).
    let rows = |edit: usize, text: &str| {
        rows(edit, text).replace("height:20px'", "height:20px;text-align:center'")
    };
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
    // Blocks far from the screen are laid out from their far records
    // (`layout::reuse::far`) without a build; every built one records.
    let far = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| view.read(cx).doc.far_blocks())
    };
    let far_before = far(&view, cx);
    update(&view, cx, &rows(300, "row 300"));
    let (again, skipped, _) = reused(&view, cx);
    let far_again = far(&view, cx) - far_before;
    assert!(
        again - recorded + far_again > 400 && skipped == 0,
        "{again} {far_again} {skipped}"
    );
    let far_before = far(&view, cx);
    update(&view, cx, &rows(300, "row edited"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    let far_edit = far(&view, cx) - far_before;
    assert!(
        skipped + far_edit > 300 && rebuilt == 0,
        "{skipped} {far_edit} {rebuilt}"
    );
}

/// Blocks near the screen are recorded with their frame output and replayed
/// in place by the next frame, the visible ones included.
#[gpui::test]
fn blocks_on_the_screen_are_replayed_from_the_previous_frame(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let (recorded, _, _) = reused(&view, cx);
    assert!(recorded > 590, "near blocks were not recorded: {recorded}");
    update(&view, cx, &rows(300, "row 300"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    let replayed = replayed(&view, cx);
    // 600 rows of 20px in a 1920x1080 test window: 54 rows are visible.
    assert!(replayed > 50, "visible blocks were rebuilt: {replayed}");
    assert!(skipped > 590 && rebuilt == 0, "{skipped} {rebuilt}");
}

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

fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_blocks())
}

#[gpui::test]
fn a_changed_block_releases_its_record_before_it_is_built(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx, &rows(300, "row 300"), &rows(300, "row edited"));
    let store = cx.update(|_, cx| view.read(cx).doc.reuse.clone());
    let recorded = store.recorded();
    assert!(recorded.len() > 400, "settled blocks were not recorded");
    let block = recorded.into_iter().find(|e| e.tag == "div").unwrap();
    assert!(
        store.same_tree(&block).is_some(),
        "an unchanged block lost its record"
    );
    // The same block after `Document::update` lost its text: the record can
    // never fit it, so it must not keep the previous subtree alive while the
    // new one is built.
    let mut changed = block.clone();
    changed.children = Vec::new().into();
    assert!(
        store.same_tree(&changed).is_none(),
        "a changed block fit its record"
    );
    assert!(
        store.same_tree(&block).is_none(),
        "the record of a changed block stayed in the store"
    );
}
