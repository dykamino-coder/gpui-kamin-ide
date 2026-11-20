//! A root scroll step replays a frame whose root background image paints on
//! the canvas: the image scrolls with the document (CSS Backgrounds 3
//! §2.11.2) while the canvas layer stays in the viewport, so the replay
//! draws what a fresh tree built at the new offset draws.

use super::*;

#[gpui::test]
fn wheel_steps_move_the_canvas_background_image_with_the_document(cx: &mut TestAppContext) {
    let html = rows(
        "<style>html{background:#fff linear-gradient(#08f,#f80) 0 0/100% 300px no-repeat}</style>",
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    for (i, dy) in [-120., -120., 120.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        let (b0, r0) = frames(&view, vcx);
        wheel_by(vcx, dy);
        frame(vcx);
        let (b1, r1) = frames(&view, vcx);
        let misses = vcx.update(|_, cx| view.read(cx).document.take_frame_misses());
        assert_eq!((b1 - b0, r1 - r0), (0, 2), "{name}: built for {misses:?}");
        let offset = root(&view, vcx).offset();
        assert_same(&name, &painted(vcx), &fresh_quads(cx, &html, offset));
    }
}
