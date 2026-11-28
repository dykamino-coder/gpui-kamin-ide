//! A panel's real prepaint environment must survive the document preparation step.

use super::tests::{LONG, frame};
use super::*;
use gpui::{Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, point, size};

struct Panel {
    page: Entity<DocumentView>,
    top: f32,
    viewport: gpui::Size<Pixels>,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("panel").pt(px(self.top)).child(
            div()
                .id("clip")
                .w(self.viewport.width)
                .h(self.viewport.height)
                .overflow_hidden()
                .font_family("Segoe UI")
                .text_size(px(13.))
                .child(self.page.clone()),
        )
    }
}

#[gpui::test]
fn embedded_frames_and_wheel_replay_without_layout(cx: &mut TestAppContext) {
    let (_one, _reader) = crate::why::test_reader();
    let html = LONG.replace("<main></main>", "<main>old text</main>");
    let (panel, cx) = cx.add_window_view(move |window, cx| Panel {
        page: DocumentView::open(
            &mut Document::builder(&html, ""),
            size(px(400.), px(300.)),
            window,
            cx,
        )
        .unwrap(),
        top: 76.,
        viewport: size(px(400.), px(300.)),
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
        let state = doc.state_rc();
        let root = state.root_scroll.as_ref().unwrap();
        assert!(root.handle.offset().y < px(0.));
        let (builds, replays, inside) = doc.frame_counters().since(&before).unwrap();
        assert_eq!((builds, inside), (0, 0));
        assert!(replays > 0);
    });

    // An in-place text edit keeps the embedding across retained generations.
    let before = cx.update(|_, cx| page.read(cx).document.frame_counters());
    cx.update(|_, cx| {
        page.update(cx, |page, cx| {
            page.document
                .update(
                    &LONG.replace("<main></main>", "<main>new longer text</main>"),
                    "",
                )
                .unwrap();
            cx.notify();
        });
    });
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
    crate::why::take();
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
    let entry = crate::why::take();
    let miss = entry
        .journal
        .causes
        .iter()
        .find(|cause| cause.code == "frame_environment.inputs")
        .expect("changed bounds must explain the environment miss");
    assert!(miss.detail.contains("bounds_old="), "{}", miss.detail);
    assert!(miss.detail.contains("bounds_new="), "{}", miss.detail);
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

#[gpui::test]
fn fractional_embedding_reuses_resolved_bounds_across_document_steps(cx: &mut TestAppContext) {
    let (panel, cx) = cx.add_window_view(|window, cx| {
        let viewport = size(px(1206.), px(728.));
        Panel {
            page: DocumentView::open(&mut Document::builder(LONG, ""), viewport, window, cx)
                .unwrap(),
            top: 76.,
            viewport,
        }
    });
    cx.simulate_scale_factor_change(1.25);
    cx.simulate_resize(size(px(1446.4), px(804.)));
    frame(cx);
    frame(cx);
    let page = cx.update(|_, cx| panel.read(cx).page.clone());
    cx.update(|_, cx| {
        let state = page.read(cx).document.state_rc();
        let bounds = state.root_scroll.as_ref().unwrap().handle.bounds();
        assert_eq!(bounds.size.width, px(1206.4));
        assert_eq!(bounds.origin.y, px(76.));
    });
    for _ in 0..4 {
        let before = cx.update(|_, cx| {
            page.update(cx, |_, cx| cx.notify());
            page.read(cx).document.frame_counters()
        });
        frame(cx);
        cx.update(|_, cx| {
            assert_eq!(
                page.read(cx).document.frame_counters().since(&before),
                Some((0, 1, 0)),
                "the rendering step substituted authored dimensions for resolved bounds"
            );
        });
    }
    // A changed viewport still rebuilds and reaches the actual scrollport.
    let before = cx.update(|_, cx| {
        panel.update(cx, |panel, cx| {
            panel.viewport.width = px(1100.);
            panel
                .page
                .update(cx, |page, cx| page.resize(panel.viewport, cx).unwrap());
            cx.notify();
        });
        page.read(cx).document.frame_counters()
    });
    frame(cx);
    cx.update(|_, cx| {
        let document = &page.read(cx).document;
        assert!(document.frame_counters().since(&before).unwrap().0 > 0);
        assert_eq!(
            document
                .state_rc()
                .root_scroll
                .as_ref()
                .unwrap()
                .handle
                .bounds()
                .size
                .width,
            px(1100.)
        );
    });
}
