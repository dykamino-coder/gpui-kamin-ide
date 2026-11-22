//! A width change settles the scroll extents it changes inside its own
//! draw: with the bars as they were, each scroll range takes its proxy's
//! new size in place (`gpui::ScrollHandle::resize_item`), as Blink updates
//! a scrollable area's range after layout. The one draw asks for no frame
//! after it, paints what a document opened at the new width paints and
//! scrolls as far.

use super::tests::{frame, open};
use gpui::TestAppContext;

fn quads(cx: &mut gpui::VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let quads = window.painted_quads().into_iter();
        quads.map(|q| format!("{q:?}")).collect()
    })
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
    frame(vcx);
    // What the first draws left to a next frame (the scrollports' first
    // extents come with new bars) is settled by the second.
    vcx.update(|window, _| window.take_frame_requests());
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.viewport = (250., 300.);
            cx.notify();
        })
    });
    frame(vcx);
    let requested = vcx.update(|window, _| window.take_frame_requests());
    assert_eq!(
        requested, 0,
        "the draw left a scroll extent to the next frame"
    );
    let after = quads(vcx);
    // The next draw lays out with the new extents: it paints the same and
    // settles nothing (the range the next layout gives is gpui's
    // `resize_item_takes_the_range_the_next_layout_gives`).
    frame(vcx);
    assert_eq!(after, quads(vcx), "the draw after it");
    assert_eq!(vcx.update(|window, _| window.take_frame_requests()), 0);
    let mut app = cx.clone();
    let (fresh, fcx) = open(&mut app, &html);
    fcx.update(|_, cx| fresh.update(cx, |view, _| view.viewport = (250., 300.)));
    frame(fcx);
    frame(fcx);
    assert_eq!(after, quads(fcx), "one draw after the width change");
}

#[gpui::test]
fn new_bars_lay_their_scrollport_out_again_and_say_why(cx: &mut TestAppContext) {
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
    frame(vcx);
    vcx.update(|window, _| window.take_frame_requests());
    let (_one, _sub) = crate::why::test_reader();
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.viewport = (250., 300.);
            cx.notify();
        })
    });
    frame(vcx);
    let journal = crate::why::take().journal;
    assert_eq!(vcx.update(|window, _| window.take_frame_requests()), 1);
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
}
