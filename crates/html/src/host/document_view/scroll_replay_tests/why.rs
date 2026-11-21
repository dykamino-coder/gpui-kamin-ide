//! The DevTools `why` journal names the refusal of a scroll replay with the
//! line that decided it and the engine line that deferred the draw.

use super::*;

#[gpui::test]
fn a_scroll_step_built_for_a_deferred_draw_names_its_producer(cx: &mut TestAppContext) {
    let html = rows("<div style='position:relative;z-index:2;height:30px;background:#00f'></div>");
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &html);
    frame(vcx);
    let (_one, _sub) = crate::why::test_reader();
    let (b0, _) = frames(&view, vcx);
    wheel_by(vcx, -120.);
    frame(vcx);
    let (b1, _) = frames(&view, vcx);
    let journal = crate::why::take().journal;
    assert!(b1 > b0, "the step replayed: {journal:?}");
    let cause = journal
        .causes
        .iter()
        .find(|c| c.code == "frame_miss.scroll_deferreddraws")
        .unwrap_or_else(|| panic!("no deferred-draw refusal in {journal:?}"));
    assert!(
        cause.site().contains("window/scroll_replay.rs:"),
        "{}",
        cause.site()
    );
    assert!(
        cause.detail.contains("paint/stacking.rs:"),
        "{}",
        cause.detail
    );
}
