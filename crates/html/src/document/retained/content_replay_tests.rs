//! Replay of box contents at any depth (`layout::reuse::content`): an edit
//! inside one table cell builds that cell's content, and the clean cells
//! replay theirs; the frame equals a fresh document's.

use super::reuse_tests::update;
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// A 3×3 table; the cell `edited` carries the attribute `data-x`.
fn page(edited: Option<usize>) -> String {
    let cells: String = (0..9)
        .map(|i| {
            let mark = if Some(i) == edited { " data-x" } else { "" };
            let row = if i % 3 == 0 { "<tr>" } else { "" };
            format!("{row}<td{mark}><span>cell {i}</span></td>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0}} td{{padding:2px;border:1px solid #333}} \
         [data-x]{{background:#ccc}}</style></head><body><table>{cells}</table></body></html>"
    )
}

fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_contents())
}

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
fn an_edit_in_one_cell_replays_the_contents_of_the_others(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, &page(None));
    frame(vcx);
    update(&view, vcx, &page(Some(0)));
    let start = replayed(&view, vcx);
    update(&view, vcx, &page(Some(4)));
    let replays = replayed(&view, vcx) - start;
    let after = scene(vcx);
    let (_, fresh) = open(cx, &page(Some(4)));
    frame(fresh);
    // Cells 0 and 4 changed; the seven others replay their content.
    assert!(replays >= 7, "clean cell contents replayed: {replays}");
    assert_eq!(
        after,
        scene(fresh),
        "the replayed frame differs from a fresh one"
    );
}

/// The same table with the rule `rule` for `[data-x]`.
fn page_with(edited: Option<usize>, rule: &str) -> String {
    page(edited).replace("[data-x]{background:#ccc}", rule)
}

#[gpui::test]
fn contents_below_a_grown_cell_replay_at_their_new_place(cx: &mut TestAppContext) {
    let tall = "[data-x]{height:40px}";
    let (view, vcx) = open(cx, &page_with(None, tall));
    frame(vcx);
    update(&view, vcx, &page_with(Some(8), tall));
    let start = replayed(&view, vcx);
    // The first cell grows: its row and every row below it move down.
    update(&view, vcx, &page_with(Some(0), tall));
    let replays = replayed(&view, vcx) - start;
    let after = scene(vcx);
    let (_, fresh) = open(cx, &page_with(Some(0), tall));
    frame(fresh);
    // Cells 0 and 8 changed; the moved and the unmoved others replay.
    assert!(replays >= 7, "clean cell contents replayed: {replays}");
    assert_eq!(
        after,
        scene(fresh),
        "the moved replay differs from a fresh build"
    );
}
