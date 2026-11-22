//! A layout settles the scroll extents and bars it finds inside its own
//! draw. With the bars as they were, each scroll range takes its proxy's
//! new size in place (`gpui::ScrollHandle::resize_item`), as Blink updates
//! a scrollable area's range after layout; new bars lay the roots out again
//! in the same draw, as Blink lays a box out again when its scrollbars
//! change. The one draw asks for no frame after it, paints what the next
//! draw and a document opened at that size paint, and scrolls as far.

use super::reuse_tests::update;
use super::tests::{frame, open};
use gpui::TestAppContext;

fn quads(cx: &mut gpui::VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter();
        quads.map(|q| format!("{q:?}")).collect()
    })
}

fn requests(cx: &mut gpui::VisualTestContext) -> usize {
    cx.update(|window, _| window.take_frame_requests())
}

/// A long root (a vertical bar) over a box wider than its small scroller
/// (a horizontal bar in it), and a scroller whose text only wraps.
fn page(body: &str) -> String {
    let text = "word ".repeat(1500);
    format!(
        "<style>body{{margin:0}}.s{{overflow:auto;height:60px;margin:4px}}</style>\
         <body{body}><div class=s><div style='width:900px;height:20px;background:#00f'></div></div>\
         <div class=s>{text}</div><p>{text}</p></body>"
    )
}

#[gpui::test]
fn the_first_draw_paints_its_bars_and_what_an_edit_paints(cx: &mut TestAppContext) {
    let html = page("");
    let (_one, _sub) = crate::why::test_reader();
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    let journal = crate::why::take().journal;
    assert_eq!(requests(vcx), 0, "the first draw left its bars to a frame");
    assert!(
        journal
            .causes
            .iter()
            .any(|c| c.code == "scroll_extent.relayout" && c.detail.starts_with("bars changed")),
        "the draw found no new bars: {journal:?}"
    );
    let first = quads(vcx);
    frame(vcx);
    assert_eq!(first, quads(vcx), "the draw after the first");
    assert_eq!(requests(vcx), 0);
    // An edit that changes no style keeps the bars: the same frame.
    update(&view, vcx, &page(" data-edit"));
    assert_eq!(first, quads(vcx), "the draw after a no-op edit");
    assert_eq!(requests(vcx), 0);
}

#[gpui::test]
fn a_width_change_settles_scroll_extents_in_its_own_draw(cx: &mut TestAppContext) {
    // Long enough that the root scrolls at both widths: its bars stay.
    let text = "word ".repeat(1500);
    let html = format!(
        "<style>body{{margin:0}}.s{{overflow:auto;height:60px;margin:4px}}</style>\
         <div class=s>{text}</div><p>{text}</p><div class=s>{text}</div>"
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    assert_eq!(requests(vcx), 0, "the first draw left its bars to a frame");
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.viewport = (250., 300.);
            cx.notify();
        })
    });
    frame(vcx);
    assert_eq!(
        requests(vcx),
        0,
        "the draw left a scroll extent to the next frame"
    );
    let after = quads(vcx);
    // The next draw lays out with the new extents: it paints the same and
    // settles nothing (the range the next layout gives is gpui's
    // `resize_item_takes_the_range_the_next_layout_gives`).
    frame(vcx);
    assert_eq!(after, quads(vcx), "the draw after it");
    assert_eq!(requests(vcx), 0);
    let mut app = cx.clone();
    let (fresh, fcx) = open(&mut app, &html);
    fcx.update(|_, cx| fresh.update(cx, |view, _| view.viewport = (250., 300.)));
    frame(fcx);
    assert_eq!(after, quads(fcx), "one draw after the width change");
}

#[gpui::test]
fn new_bars_lay_the_roots_out_again_in_the_same_draw_and_say_why(cx: &mut TestAppContext) {
    // At 400px the 300px box fits; at 250px it overflows and its scrollport
    // gets a horizontal bar, which takes room from its content box.
    let text = "word ".repeat(1500);
    let html = format!(
        "<style>body{{margin:0}}.s{{overflow:auto;height:60px;margin:4px}}</style>\
         <div class=s><div style='width:300px;height:20px;background:#00f'></div></div><p>{text}</p>"
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    requests(vcx);
    let (_one, _sub) = crate::why::test_reader();
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.viewport = (250., 300.);
            cx.notify();
        })
    });
    frame(vcx);
    let journal = crate::why::take().journal;
    assert_eq!(requests(vcx), 0, "the new bar waited for a frame");
    let cause = journal
        .causes
        .iter()
        .find(|c| c.code == "scroll_extent.relayout")
        .unwrap_or_else(|| panic!("no relayout cause in {journal:?}"));
    assert!(cause.detail.starts_with("bars changed"), "{}", cause.detail);
    assert!(
        cause.site().contains("scroll_extent/settle.rs:"),
        "{}",
        cause.site()
    );
    let after = quads(vcx);
    let mut app = cx.clone();
    let (fresh, fcx) = open(&mut app, &html);
    fcx.update(|_, cx| fresh.update(cx, |view, _| view.viewport = (250., 300.)));
    frame(fcx);
    assert_eq!(after, quads(fcx), "one draw after the width change");
}
