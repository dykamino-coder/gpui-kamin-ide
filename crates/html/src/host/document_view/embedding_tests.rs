//! A panel's real prepaint environment must survive the document preparation step.

use super::tests::{LONG, frame};
use super::*;
use gpui::{Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, point, size};

struct Panel {
    page: Entity<DocumentView>,
    top: f32,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("panel").pt(px(self.top)).child(
            div()
                .id("clip")
                .w(px(400.))
                .h(px(300.))
                .overflow_hidden()
                .font_family("Segoe UI")
                .text_size(px(13.))
                .child(self.page.clone()),
        )
    }
}

#[gpui::test]
fn embedded_frames_and_wheel_replay_without_layout(cx: &mut TestAppContext) {
    let (panel, cx) = cx.add_window_view(|window, cx| Panel {
        page: DocumentView::open(
            &mut Document::builder(LONG, ""),
            size(px(400.), px(300.)),
            window,
            cx,
        )
        .unwrap(),
        top: 76.,
    });
    frame(cx);
    frame(cx);
    let page = cx.update(|_, cx| panel.read(cx).page.clone());
    for _ in 0..3 {
        let before = cx.update(|_, cx| page.read(cx).document.frame_counters());
        frame(cx);
        cx.update(|_, cx| {
            let after = page.read(cx).document.frame_counters();
            assert_eq!(after.since(&before), Some((0, 1, 0)));
        });
    }
    let before = cx.update(|_, cx| page.read(cx).document.frame_counters());
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(50.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    frame(cx);
    cx.update(|_, cx| {
        let doc = &page.read(cx).document;
        let root = doc.state_rc().root_scroll.as_ref().unwrap();
        assert!(root.handle.offset().y < px(0.));
        let (builds, replays, inside) = doc.frame_counters().since(&before).unwrap();
        assert_eq!((builds, inside), (0, 0));
        assert!(replays > 0);
    });

    // A new generation still uses the element's embedding for preparation.
    cx.update(|_, cx| {
        page.update(cx, |page, cx| {
            page.document
                .update(&format!("{LONG}<p>new text</p>"), "")
                .unwrap();
            cx.notify();
        });
    });
    let before = cx.update(|_, cx| page.read(cx).document.frame_counters());
    frame(cx);
    cx.update(|_, cx| {
        let (builds, _, inside) = page
            .read(cx)
            .document
            .frame_counters()
            .since(&before)
            .unwrap();
        assert!(builds > 0);
        assert_eq!(inside, 0);
    });

    // Moving the panel must reject the old origin/clip rather than replay it.
    let before = cx.update(|_, cx| page.read(cx).document.frame_counters());
    cx.update(|_, cx| {
        panel.update(cx, |panel, cx| {
            panel.top = 96.;
            cx.notify();
        })
    });
    frame(cx);
    cx.update(|_, cx| {
        let (builds, _, _) = page
            .read(cx)
            .document
            .frame_counters()
            .since(&before)
            .unwrap();
        assert!(builds > 0, "changed embedding replayed stale geometry");
    });
    frame(cx);
    let before = cx.update(|_, cx| page.read(cx).document.frame_counters());
    frame(cx);
    cx.update(|_, cx| {
        assert_eq!(
            page.read(cx).document.frame_counters().since(&before),
            Some((0, 1, 0))
        );
    });
}
