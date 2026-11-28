//! A host's toolbar and clip must not turn unchanged window frames into document builds.

use super::tests::{LONG, frame};
use super::*;
use gpui::{Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, point, size};

struct Shell {
    page: Entity<DocumentView>,
    left: f32,
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("shell").pt(px(76.)).pl(px(self.left)).child(
            div()
                .w(px(400.))
                .h(px(300.))
                .overflow_hidden()
                .child(self.page.clone()),
        )
    }
}

#[gpui::test]
fn embedded_window_frames_and_wheel_reuse_but_host_moves_revalidate(cx: &mut TestAppContext) {
    let (shell, cx) = cx.add_window_view(|window, cx| Shell {
        page: DocumentView::open(
            &mut Document::builder(LONG, ""),
            size(px(400.), px(300.)),
            window,
            cx,
        )
        .unwrap(),
        left: 32.,
    });
    frame(cx);
    frame(cx);
    let page = cx.update(|_, cx| shell.read(cx).page.clone());
    let (_one, _reader) = crate::why::test_reader();
    let built = cx.update(|_, cx| page.read(cx).document.retained_frames().0);
    for _ in 0..4 {
        frame(cx);
    }
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(100.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    frame(cx);
    cx.update(|_, cx| {
        let doc = &page.read(cx).document;
        assert_eq!(doc.retained_frames().0, built);
        assert!(
            doc.state_rc()
                .root_scroll
                .as_ref()
                .unwrap()
                .handle
                .offset()
                .y
                < px(0.)
        );
    });
    let entry = crate::why::take();
    assert!(entry.journal.window_frames >= 5, "{entry:?}");
    assert_eq!(entry.journal.window_document_builds, 0, "{entry:?}");
    assert_eq!(crate::why::to_json(&entry)["window_frames"]["builds"], 0);

    // The predicted placement cannot authorize replay after the host actually moved.
    let old = super::paint_compare::quads(cx);
    cx.update(|_, cx| {
        shell.update(cx, |shell, cx| {
            shell.left = 64.;
            cx.notify();
        })
    });
    frame(cx);
    assert_ne!(super::paint_compare::quads(cx), old);
    cx.update(|_, cx| assert!(page.read(cx).document.retained_frames().0 > built));
    crate::why::take();
    frame(cx);
    frame(cx);
    let entry = crate::why::take();
    assert_eq!(entry.journal.window_document_builds, 0, "{entry:?}");
}
