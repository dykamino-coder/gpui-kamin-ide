//! A document opened at a scroll offset (a restored position or a fragment,
//! CSSOM View §7) shows there in its first draw: the root scroller keeps the
//! offset asked for until its range is measured (the prepaint does not clamp
//! it to a range not known yet), then clamps it to that range, and the
//! DevTools `why` journal names the clamp (`scroll.clamped`).

use super::tests::{View, frame, open_document};
use gpui::{Entity, TestAppContext, VisualTestContext};

/// Twenty bands of distinct colours, 2000px tall in a 300px high view.
fn page() -> String {
    let bands: String = (0..20)
        .map(|i| {
            format!(
                "<div style='height:100px;background:rgb({},0,{})'></div>",
                i * 12,
                255 - i * 12
            )
        })
        .collect();
    format!("<style>body{{margin:0}}</style>{bands}")
}

fn opened(cx: &mut TestAppContext) -> (Entity<View>, &mut VisualTestContext) {
    let doc = crate::Document::builder(&page(), "")
        .build_root_scroller()
        .unwrap();
    open_document(cx, doc)
}

fn scroll(view: &Entity<View>, vcx: &mut VisualTestContext, y: f32) {
    vcx.update(|_, cx| view.update(cx, |view, _| view.doc.set_root_scroll_offset((0., y))));
}

fn offset(view: &Entity<View>, vcx: &mut VisualTestContext) -> Option<(f32, f32)> {
    vcx.update(|_, cx| view.read(cx).doc.root_scroll_offset())
}

fn quads(vcx: &mut VisualTestContext) -> Vec<String> {
    vcx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .map(|q| format!("{q:?}"))
            .collect()
    })
}

fn clamps(entry: &crate::why::Entry) -> Vec<String> {
    let clamps = entry.frames.iter().filter(|e| e.kind == "scroll.clamped");
    clamps.map(|e| e.detail.clone()).collect()
}

/// The frame of a document opened at 0 and then scrolled to `y`.
fn scrolled_to(cx: &mut TestAppContext, y: f32) -> Vec<String> {
    let mut app = cx.clone();
    let (view, vcx) = opened(&mut app);
    frame(vcx);
    scroll(&view, vcx, y);
    frame(vcx);
    assert_eq!(offset(&view, vcx), Some((0., y)));
    quads(vcx)
}

#[gpui::test]
fn a_document_opened_at_an_offset_shows_there_in_its_first_draw(cx: &mut TestAppContext) {
    let (_one, _sub) = crate::why::test_reader();
    let mut app = cx.clone();
    let (view, vcx) = opened(&mut app);
    scroll(&view, vcx, 120.);
    frame(vcx);
    let entry = crate::why::take();
    assert_eq!(
        offset(&view, vcx),
        Some((0., 120.)),
        "the first draw moved the offset asked for"
    );
    assert_eq!(
        clamps(&entry),
        Vec::<String>::new(),
        "a clamp moved the offset"
    );
    let first = quads(vcx);
    assert_eq!(
        first,
        scrolled_to(cx, 120.),
        "the first draw differs from a document scrolled there"
    );
}

#[gpui::test]
fn an_offset_past_the_range_is_clamped_to_it_once_measured(cx: &mut TestAppContext) {
    let (_one, _sub) = crate::why::test_reader();
    let mut app = cx.clone();
    let (view, vcx) = opened(&mut app);
    frame(vcx);
    crate::why::take();
    // A script's scroll past the end: the range the draw measures clamps it.
    scroll(&view, vcx, 100_000.);
    frame(vcx);
    let entry = crate::why::take();
    // 2000px of bands in a 300px high view.
    assert_eq!(
        offset(&view, vcx),
        Some((0., 1700.)),
        "the offset is not the measured range's end"
    );
    let clamps = clamps(&entry);
    assert!(
        clamps
            .iter()
            .any(|clamp| clamp.contains("-100000") && clamp.contains("range")),
        "the journal does not name the clamp ({} events dropped): {clamps:?}",
        entry.frames_dropped
    );
    let first = quads(vcx);
    assert_eq!(
        first,
        scrolled_to(cx, 1700.),
        "the clamped draw differs from a document scrolled there"
    );
}
