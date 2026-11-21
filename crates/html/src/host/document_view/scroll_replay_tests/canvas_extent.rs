//! A repeated root background image covers the whole canvas, past a root
//! box shorter than the viewport (CSS Backgrounds 3 §2.11.2), when it is
//! painted under the follower of the root scroller: on the frame a build
//! draws and on every replayed wheel step. (A fresh document opened at the
//! step's offset (`fresh_quads`) opens at zero for this page, so a step is
//! checked by its coverage, not against a fresh frame.)

use super::*;

/// The lowest device row the painted gradient quads reach.
fn gradient_bottom(painted: &[Painted]) -> f32 {
    painted
        .iter()
        .filter(|(key, _, area)| key.contains("LinearGradient") && !area.is_empty())
        .map(|(_, _, area)| area.bottom().0)
        .fold(0., f32::max)
}

#[gpui::test]
fn a_repeated_canvas_image_covers_the_viewport_below_a_short_root(cx: &mut TestAppContext) {
    let html = rows(
        "<style>html{height:100px;background:#fff linear-gradient(#08f,#f80) 0 0/100% 50px}</style>",
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    // The view `open` makes is 300 px high (device rows at the window's scale).
    let bottom = vcx.update(|window, _| 300. * window.scale_factor());
    let reached = gradient_bottom(&painted(vcx));
    assert!(
        reached >= bottom,
        "built frame: tiles end at {reached} of {bottom}"
    );
    for (i, dy) in [-120., -120., 120.].into_iter().enumerate() {
        let name = format!("wheel {i} by {dy}");
        let (b0, r0) = frames(&view, vcx);
        wheel_by(vcx, dy);
        frame(vcx);
        let (b1, r1) = frames(&view, vcx);
        assert_eq!((b1 - b0, r1 - r0), (0, 2), "{name}: a step replays");
        let after = painted(vcx);
        let reached = gradient_bottom(&after);
        assert!(
            reached >= bottom,
            "{name}: tiles end at {reached} of {bottom}"
        );
    }
}
