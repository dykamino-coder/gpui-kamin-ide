//! Replay of box contents holding `<img>` (`layout::reuse::plain::store_image`):
//! an edit in one cell replays the others with their images while each image's
//! request keeps its state; an image taken off the document frees its request.

use super::layered_replay_tests::scene;
use super::reuse_tests::update;
use super::tests::{View, frame, open};
use gpui::{TestAppContext, VisualTestContext};

/// A 3×3 table of fixed layout, every cell with an image (`images`); the
/// text of the cell `edited` changes, so the frame is built.
fn page(edited: Option<usize>, images: bool) -> String {
    let cells: String = (0..9)
        .map(|i| {
            let word = if Some(i) == edited { "edit" } else { "cell" };
            let row = if i % 3 == 0 { "<tr>" } else { "" };
            let image = if images {
                format!("<img src=missing-{i}.png width=12 height=12>")
            } else {
                String::new()
            };
            format!("{row}<td><span>{word} {i}</span> {image}</td>")
        })
        .collect();
    format!(
        "<!doctype html><html><head><style>body{{margin:0}} td{{padding:2px;border:1px solid #333}} \
         table{{table-layout:fixed;width:600px}}</style></head><body><table>{cells}</table></body></html>"
    )
}

fn replayed(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> u64 {
    cx.update(|_, cx| view.read(cx).doc.replayed_contents())
}

fn requests(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| view.read(cx).doc.state.values.images.borrow().requests())
}

#[gpui::test]
fn an_edit_in_one_cell_replays_the_other_cells_with_images(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, &page(None, true));
    frame(vcx);
    update(&view, vcx, &page(Some(0), true));
    let start = replayed(&view, vcx);
    let before = requests(&view, vcx);
    update(&view, vcx, &page(Some(4), true));
    let replays = replayed(&view, vcx) - start;
    let after = scene(vcx);
    let held = requests(&view, vcx);
    let (_, fresh) = open(cx, &page(Some(4), true));
    frame(fresh);
    // Cells 0 and 4 changed; the seven others replay their content.
    assert!(
        replays >= 7,
        "clean cell contents with images replayed: {replays}"
    );
    assert_eq!(
        after,
        scene(fresh),
        "the replayed frame differs from a fresh one"
    );
    // The same images keep the same requests: none is made again or kept twice.
    assert!(before > 0 && held == before, "requests {before} -> {held}");
}

#[gpui::test]
fn images_taken_off_the_document_free_their_requests(cx: &mut TestAppContext) {
    let (view, vcx) = open(cx, &page(None, true));
    frame(vcx);
    update(&view, vcx, &page(Some(1), true));
    for edited in 2..5 {
        update(&view, vcx, &page(Some(edited), false));
    }
    assert_eq!(
        requests(&view, vcx),
        0,
        "requests of removed images were kept"
    );
}
