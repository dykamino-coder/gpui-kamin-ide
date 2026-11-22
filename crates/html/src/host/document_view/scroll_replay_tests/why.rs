//! The DevTools `why` journal names the refusal of a scroll replay with the
//! line that decided it. (Which deferred draw refused, and where it was
//! made, is the gpui fork's check: `window/scroll_deferred/tests.rs`; the
//! engine's positioned and sticky boxes replay with their scroll node.)

use super::*;

#[gpui::test]
fn a_scroll_step_built_for_a_3d_plane_names_the_refusal(cx: &mut TestAppContext) {
    // A plane of a 3D context: its group cannot move with a scroll node.
    let html = rows(
        "<div style='perspective:100px'><div style='transform:rotateY(20deg);opacity:.5'>x</div></div>",
    );
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    let (_one, _sub) = crate::why::test_reader();
    let (b0, _) = frames(&view, vcx);
    wheel_by(vcx, -120.);
    frame(vcx);
    let (b1, _) = frames(&view, vcx);
    let journal = crate::why::take().journal;
    assert!(b1 > b0, "the step was built: {journal:?}");
    let cause = journal
        .causes
        .iter()
        .find(|c| c.code.starts_with("frame_miss.scroll_"))
        .unwrap_or_else(|| panic!("no scroll refusal in {journal:?}"));
    assert!(
        cause.site().contains("window/scroll_replay.rs:"),
        "{} at {}",
        cause.code,
        cause.site()
    );
}
