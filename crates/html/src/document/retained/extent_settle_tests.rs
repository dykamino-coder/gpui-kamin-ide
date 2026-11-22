//! A layout settles the scroll extents and bars it finds inside its own
//! draw. Layout decides the bars of each `auto` scrollport and lays only
//! that box out again when they change, as Blink `BlockNode::Layout` does
//! when `ComputeScrollbars` changes; each scroll range then takes its
//! proxy's new size in place (`gpui::ScrollHandle::resize_item`), as Blink
//! updates a scrollable area's range after layout. The one draw asks for no frame after it, paints what the next
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
    // Layout decides the new bars itself: no scrollport is laid out again.
    assert!(
        journal
            .causes
            .iter()
            .any(|c| c.code == "scroll_extent.layout_bars"),
        "the draw found no new bars: {journal:?}"
    );
    assert!(
        !journal
            .causes
            .iter()
            .any(|c| c.code == "scroll_extent.relayout"),
        "a scrollport was laid out again: {journal:?}"
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
fn layout_decides_new_bars_in_the_same_draw_and_says_why(cx: &mut TestAppContext) {
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
    // Layout itself lays the box out again with the new bar and says so.
    let cause = journal
        .causes
        .iter()
        .find(|c| c.code == "scroll_extent.layout_bars")
        .unwrap_or_else(|| panic!("no layout bars cause in {journal:?}"));
    assert!(
        cause.detail.contains("decided in layout"),
        "{}",
        cause.detail
    );
    assert!(
        !journal
            .causes
            .iter()
            .any(|c| c.code == "scroll_extent.relayout"),
        "a scrollport was laid out again: {journal:?}"
    );
    let after = quads(vcx);
    let mut app = cx.clone();
    let (fresh, fcx) = open(&mut app, &html);
    fcx.update(|_, cx| fresh.update(cx, |view, _| view.viewport = (250., 300.)));
    frame(fcx);
    assert_eq!(after, quads(fcx), "one draw after the width change");
}

#[gpui::test]
fn a_bar_that_brings_the_other_settles_in_the_same_draw(cx: &mut TestAppContext) {
    // The first layout guesses the root's vertical bar: narrowed by it, the
    // scroller's two blocks wrap and it gets both bars (the wide box gives
    // the horizontal one). The short root drops the guessed bar, the
    // scroller widens, its blocks share a line and its vertical bar goes:
    // the scroller's bars change twice, a third layout.
    let html = "<style>body{margin:0}.s{overflow:auto;height:80px}\
         .b{display:inline-block;width:198px;height:30px;vertical-align:top}</style>\
         <div class=s><div style='width:900px;height:20px'></div>\
         <div><span class=b></span><span class=b></span></div></div>";
    let mut app = cx.clone();
    let (_view, vcx) = open(&mut app, html);
    frame(vcx);
    assert_eq!(requests(vcx), 0, "the second bar waited for a frame");
    let first = quads(vcx);
    frame(vcx);
    assert_eq!(first, quads(vcx), "the draw after the first");
    assert_eq!(requests(vcx), 0);
}

/// A padded preformatted scroller whose lines fit, with a positioned link at
/// its padding edge, decides its bars in layout, and the measured range
/// agrees: end padding follows only the in-flow content (CSS Overflow 3 §2.2,
/// as Chrome's `scrollWidth == clientWidth`).
#[gpui::test]
fn a_padded_scroller_whose_lines_fit_needs_no_second_layout(cx: &mut TestAppContext) {
    let html = "<style>body{margin:0}pre{padding:1em;margin:.5em 0;overflow:auto;white-space:pre}</style>\
                <pre>short <span>line</span>\nand another</pre><pre><code>code <b>in</b> code</code></pre><div style=\"width:3000px;height:3000px\"></div>\
                <div class=example style=\"position:relative\"><pre class=\"highlight lang-css\" style=\"position:relative\"><a style=\"position:absolute;top:0;right:0;width:30px;height:26px\"></a>h1 {\n  font-weight: bold;\n}\n</pre></div>";
    let (_one, _sub) = crate::why::test_reader();
    let mut app = cx.clone();
    let (_view, vcx) = open(&mut app, html);
    frame(vcx);
    let journal = crate::why::take().journal;
    let relayouts: Vec<_> = journal
        .causes
        .iter()
        .filter(|c| c.code == "scroll_extent.relayout")
        .map(|c| c.detail.clone())
        .collect();
    assert!(relayouts.is_empty(), "laid out again: {relayouts:#?}");
    assert_eq!(requests(vcx), 0);
}

/// A resizable padded scroller around a responsive grid (the layout of a
/// common CSS demo page): whatever its measured range finds, the bars layout
/// decided hold, so the page goes quiet after its first draw.
#[gpui::test]
fn bars_layout_decided_go_quiet_after_the_first_draw(cx: &mut TestAppContext) {
    let html = "<style>*{box-sizing:border-box}body{margin:16px;line-height:1.65}\
        .demo{border:1px solid;padding:1.5rem;max-width:calc(100vi - 2rem);overflow:auto;\
        position:relative;resize:horizontal}ul{list-style:none;margin:0;padding:0;display:grid;\
        gap:1rem;grid-template-columns:repeat(auto-fit,minmax(min(100%,15ch),1fr))}\
        li{border:1px dashed;padding:1rem;display:grid;place-content:center;font-size:2rem}</style>\
        <div class=demo><ul><li>Item 1</li><li>Item 2</li><li>Item 3</li></ul></div>";
    let (_one, _sub) = crate::why::test_reader();
    let mut app = cx.clone();
    let (_view, vcx) = open(&mut app, html);
    frame(vcx);
    let journal = crate::why::take().journal;
    assert_eq!(requests(vcx), 0, "the first draw asked for another");
    assert!(
        !journal
            .causes
            .iter()
            .any(|c| c.code == "scroll_extent.relayout"),
        "a scrollport was laid out again: {journal:?}"
    );
    let first = quads(vcx);
    for _ in 0..3 {
        frame(vcx);
        assert_eq!(requests(vcx), 0, "an idle draw asked for another");
        assert_eq!(first, quads(vcx), "an idle draw painted something else");
    }
}
