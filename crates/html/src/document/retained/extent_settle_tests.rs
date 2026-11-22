//! A width change settles the scroll extents and bars it changes inside its
//! own draw: `ScrollArea` retries the draw's layout with them
//! (`Frame::settle_in_draw`), as Blink settles scrollbars and the scroll
//! extent inside the layout of the same lifecycle. The one draw asks for no
//! frame after it and paints what a document opened at the new width paints.

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
    let text = "word ".repeat(300);
    let html = format!(
        "<style>body{{margin:0}}.s{{overflow:auto;height:60px;margin:4px}}</style>\
         <div class=s>{text}</div><p>{text}</p><div class=s>{text}</div>"
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    frame(vcx);
    vcx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.viewport = (250., 300.);
            cx.notify();
        })
    });
    frame(vcx);
    let requested = vcx.update(|window, _| window.frame_requested_after_draw());
    assert!(
        !requested,
        "the draw left a scroll extent to the next frame"
    );
    let after = quads(vcx);
    let mut app = cx.clone();
    let (fresh, fcx) = open(&mut app, &html);
    fcx.update(|_, cx| fresh.update(cx, |view, _| view.viewport = (250., 300.)));
    frame(fcx);
    frame(fcx);
    assert_eq!(after, quads(fcx), "one draw after the width change");
}
