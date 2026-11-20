//! A rendered viewport reads the native scroll position even without input dispatch.
use super::tests::{frame, open};
use gpui::{TestAppContext, VisualTestContext, point, px};

const BANDS: &str = "<style>body{margin:0}section{height:120px;background:red}
    article{height:180px;background:lime}main{height:800px;background:blue}</style>
    <section></section><article></article><main></main>";

fn scene(cx: &mut VisualTestContext) -> Vec<gpui::Quad> {
    cx.update(|window, _| {
        window
            .painted_quads()
            .into_iter()
            .map(|mut quad| {
                quad.order = 0;
                quad
            })
            .collect()
    })
}

#[gpui::test]
fn scripted_viewport_scroll_matches_fresh_and_then_replays(cx: &mut TestAppContext) {
    let reference = {
        let (view, cx) = open(cx, BANDS);
        cx.update(|_, cx| {
            view.update(cx, |view, _| {
                // A new visual tree must build even on an engine that ignores scroll offsets.
                view.document
                    .update(&format!("{BANDS}<aside></aside>"), "")
                    .unwrap();
                view.document
                    .state_rc()
                    .root_scroll
                    .as_ref()
                    .unwrap()
                    .handle
                    .set_offset(point(px(0.), px(-170.)));
            })
        });
        // A new document resolves its native scrollbar gutter on the following frame.
        frame(cx);
        frame(cx);
        scene(cx)
    };
    let (view, cx) = open(cx, BANDS);
    let before = scene(cx);
    assert_ne!(reference, before, "scroll must expose different bands");
    let counts =
        |_: &mut gpui::Window, app: &mut gpui::App| view.read(app).document.retained_frames();
    let initial = cx.update(counts);
    cx.update(|_, cx| {
        view.read(cx)
            .document
            .state_rc()
            .root_scroll
            .as_ref()
            .unwrap()
            .handle
            .set_offset(point(px(0.), px(-170.)))
    });
    frame(cx);
    assert_eq!(
        scene(cx),
        reference,
        "scripted scroll replayed the old scene"
    );
    let moved = cx.update(counts);
    assert!(moved.0 > initial.0);
    frame(cx);
    assert_eq!(cx.update(counts), (moved.0, moved.1 + 1));
}
