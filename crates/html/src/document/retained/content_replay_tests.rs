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

/// Nine blocks in a wrapper (not the root flow); the block `edited` carries
/// the attribute `data-x`, which `rule` styles.
fn blocks(edited: Option<usize>, rule: &str) -> String {
    let items: String = (0..9)
        .map(|i| {
            let mark = if Some(i) == edited { " data-x" } else { "" };
            format!("<div class=item{mark}><span>block {i}</span> <b>bold</b></div>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0}} .item{{padding:2px;border:1px solid #333}} \
         {rule}</style></head><body><div id=wrap>{items}</div></body></html>"
    )
}

#[gpui::test]
fn contents_below_a_grown_block_replay_at_their_new_place(cx: &mut TestAppContext) {
    let tall = "[data-x]{height:40px}";
    let (view, vcx) = open(cx, &blocks(None, tall));
    frame(vcx);
    update(&view, vcx, &blocks(Some(8), tall));
    let start = replayed(&view, vcx);
    // The first block grows: every block below it moves down.
    update(&view, vcx, &blocks(Some(0), tall));
    let replays = replayed(&view, vcx) - start;
    let after = scene(vcx);
    let (_, fresh) = open(cx, &blocks(Some(0), tall));
    frame(fresh);
    // Blocks 0 and 8 changed; the moved and the unmoved others replay.
    assert!(replays >= 7, "clean block contents replayed: {replays}");
    assert_eq!(
        after,
        scene(fresh),
        "the moved replay differs from a fresh build"
    );
}

/// A table whose last cell, under `html.alt`, sets the width of its column.
fn columns(alt: bool) -> String {
    let mut cells = String::new();
    for r in 0..4 {
        cells.push_str("<tr>");
        for c in 0..4 {
            let deep = if r == 3 && c == 3 { " data-deep" } else { "" };
            cells.push_str(&format!("<td><span{deep}>{r}-{c} a b c</span></td>"));
        }
        cells.push_str("</tr>");
    }
    let class = if alt { " class=alt" } else { "" };
    format!(
        "<!doctype html><html{class}><head><style>body{{margin:0}} table{{width:300px}} \
         span{{display:inline-block}} html.alt [data-deep]{{display:block;width:200px}}\
         </style></head><body><div><table>{cells}</table></div></body></html>"
    )
}

#[gpui::test]
fn cells_of_a_table_whose_columns_change_are_built(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, &columns(false));
    frame(vcx);
    update(&view, vcx, &columns(true));
    update(&view, vcx, &columns(false));
    let rebuilt = |vcx: &mut VisualTestContext| cx_rebuilt(&view, vcx);
    let (start, again) = (replayed(&view, vcx), rebuilt(vcx));
    // One cell widens its column: every cell may get another width, so none
    // is replayed (and refused after layout, which would build the frame twice).
    update(&view, vcx, &columns(true));
    assert_eq!(replayed(&view, vcx), start);
    assert_eq!(rebuilt(vcx), again, "no frame was built again");
    let after = scene(vcx);
    let (_, fresh) = open(cx, &columns(true));
    frame(fresh);
    assert_eq!(after, scene(fresh));
}
/// Frames built again after their check refused a replay.
fn cx_rebuilt(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.reused_blocks().2)
}
