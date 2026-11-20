//! TEMP diag (cls20): first frames of a real page; not for commit.

use super::tests::{frame, open_document};
use crate::Document;
use gpui::TestAppContext;

#[gpui::test]
fn dbg_real_page_frames(cx: &mut TestAppContext) {
    let Some(path) = std::env::var_os("KAMIN_DBG_PAGE") else {
        return;
    };
    let html = std::fs::read_to_string(path).unwrap();
    let doc = Document::builder(&html, "").build_root_scroller().unwrap();
    let (view, cx) = open_document(cx, doc);
    for i in 0..3 {
        let t = std::time::Instant::now();
        frame(cx);
        let leaves = cx.update(|_, cx| view.read(cx).doc.offscreen_leaves());
        eprintln!("DBGFRAME {i} leaves={leaves} ms={}", t.elapsed().as_millis());
    }
}
