//! Lifecycle and input checks for the real document viewport, without an outer scroller.

use super::*;
use gpui::{
    Entity, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point,
    size,
};

const LONG: &str =
    "<style>body{margin:0}main{height:1800px;width:1200px;background:blue}</style><main></main>";

fn open<'a>(
    cx: &'a mut TestAppContext,
    html: &str,
) -> (Entity<DocumentView>, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let document = Document::builder(html, "").build_for_view(true).unwrap();
        let view = DocumentView::new(document, size(px(400.), px(300.)), window, cx);
        view.focus.focus(window, cx);
        view
    });
    frame(cx);
    frame(cx);
    (view, cx)
}

fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn offset(view: &Entity<DocumentView>, cx: &mut VisualTestContext) -> gpui::Point<Pixels> {
    cx.update(|_, cx| {
        view.read(cx)
            .document
            .state_rc()
            .root_scroll
            .as_ref()
            .unwrap()
            .handle
            .offset()
    })
}

fn wheel(cx: &mut VisualTestContext, x: f32, y: f32) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(x), px(y)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
        modifiers: Modifiers::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
}

#[gpui::test]
fn plain_root_scrolls_with_wheel_and_keys(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    assert_eq!(offset(&view, cx).y, px(0.));
    wheel(cx, 20., 20.);
    frame(cx);
    assert!(offset(&view, cx).y < px(0.));
    cx.simulate_keystrokes("end");
    frame(cx);
    assert!(offset(&view, cx).y < px(-1000.));
    cx.simulate_keystrokes("home");
    frame(cx);
    assert_eq!(offset(&view, cx).y, px(0.));
    cx.simulate_keystrokes("pagedown");
    frame(cx);
    assert!(offset(&view, cx).y < px(-100.));
}

#[gpui::test]
fn embedded_resize_relayouts_without_reparsing(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    let before = cx.update(|_, cx| view.read(cx).document.salt());
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.resize(size(px(250.), px(200.)), cx).unwrap()
        })
    });
    frame(cx);
    frame(cx);
    cx.update(|_, cx| {
        let view = view.read(cx);
        assert_eq!(view.document.salt(), before);
        let handle = &view
            .document
            .state_rc()
            .root_scroll
            .as_ref()
            .unwrap()
            .handle
            .clone();
        assert_eq!(handle.bounds().size, size(px(250.), px(200.)));
    });
}

#[gpui::test]
fn resize_reevaluates_media_queries_of_the_document(cx: &mut TestAppContext) {
    let html = "<style>body{margin:0}main{height:1800px}
        @media (max-width: 300px){main{height:100px}}</style><main></main>";
    let (view, cx) = cx.add_window_view(|window, cx| {
        EmbeddedPage(
            DocumentView::open(
                &mut Document::builder(html, ""),
                size(px(400.), px(300.)),
                window,
                cx,
            )
            .unwrap(),
        )
    });
    frame(cx);
    frame(cx);
    let page = cx.update(|_, cx| view.read(cx).0.clone());
    let range = |page: &Entity<DocumentView>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            let root = page.read(cx).document.state_rc().root_scroll.clone();
            root.unwrap().handle.max_offset().y
        })
    };
    assert!(range(&page, cx) > px(1000.));
    cx.update(|_, cx| {
        page.update(cx, |page, cx| {
            page.resize(size(px(250.), px(300.)), cx).unwrap()
        })
    });
    frame(cx);
    frame(cx);
    assert_eq!(range(&page, cx), px(0.));
    cx.update(|_, cx| {
        page.update(cx, |page, cx| {
            page.resize(size(px(500.), px(300.)), cx).unwrap()
        })
    });
    frame(cx);
    frame(cx);
    assert!(range(&page, cx) > px(1000.));
}

#[gpui::test]
fn root_offset_survives_rebuild_and_clamps_when_content_shrinks(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    cx.simulate_keystrokes("pagedown");
    frame(cx);
    let before = offset(&view, cx);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document
                .update(&format!("{LONG}<footer>new node</footer>"), "")
                .unwrap();
            cx.notify();
        })
    });
    frame(cx);
    frame(cx);
    assert_eq!(offset(&view, cx), before);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document.update("<p>short page</p>", "").unwrap();
            cx.notify();
        })
    });
    frame(cx);
    frame(cx);
    assert_eq!(offset(&view, cx).y, px(0.));
}

#[gpui::test]
fn hidden_root_does_not_scroll_with_keys(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &format!("{LONG}<style>html{{overflow:hidden}}</style>"));
    cx.simulate_keystrokes("pagedown");
    frame(cx);
    assert_eq!(offset(&view, cx).y, px(0.));
}

#[gpui::test]
fn hiding_a_previously_scrollable_root_disables_keys(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document
                .update(&format!("{LONG}<style>html{{overflow:hidden}}</style>"), "")
                .unwrap();
            cx.notify();
        })
    });
    frame(cx);
    frame(cx);
    cx.simulate_keystrokes("end");
    frame(cx);
    assert_eq!(offset(&view, cx).y, px(0.));
}

#[gpui::test]
fn stable_frames_reuse_the_document_prepared_outside_draw(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    let before = cx.update(|_, cx| view.read(cx).document.builds_inside_draw());
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.resize(size(px(350.), px(250.)), cx).unwrap()
        })
    });
    frame(cx);
    frame(cx);
    cx.update(|_, cx| assert_eq!(view.read(cx).document.builds_inside_draw(), before));
}

#[gpui::test]
fn nested_wheel_scrolls_the_inner_box_before_the_document(cx: &mut TestAppContext) {
    let html = "<style>body{margin:0}section{width:200px;height:100px;overflow:auto}
        article{height:800px}main{height:1800px}</style>
        <section><article>inner</article></section><main></main>";
    let (view, cx) = open(cx, html);
    wheel(cx, 20., 20.);
    frame(cx);
    assert_eq!(offset(&view, cx).y, px(0.));
    // At the inner boundary, the next wheel event must reach the document.
    for _ in 0..8 {
        wheel(cx, 20., 20.);
        frame(cx);
    }
    assert!(offset(&view, cx).y < px(0.));
    cx.simulate_keystrokes("home");
    frame(cx);
    wheel(cx, 250., 150.);
    frame(cx);
    assert!(offset(&view, cx).y < px(0.));
}

#[gpui::test]
fn foreign_focus_does_not_scroll_the_root(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, LONG);
    let foreign = cx.update(|window, cx| {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        focus
    });
    cx.simulate_keystrokes("end");
    frame(cx);
    assert_eq!(offset(&view, cx).y, px(0.));
    drop(foreign);
}

#[gpui::test]
fn contained_root_still_has_a_document_scrollport(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, &format!("{LONG}<style>html{{contain:paint}}</style>"));
    wheel(cx, 20., 20.);
    frame(cx);
    assert!(offset(&view, cx).y < px(0.));
}

struct EmbeddedPage(Entity<DocumentView>);

impl Render for EmbeddedPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("embedding-parent").child(self.0.clone())
    }
}

#[gpui::test]
fn nested_view_reuses_prepared_document_after_embedding(cx: &mut TestAppContext) {
    let (root, cx) = cx.add_window_view(|window, cx| {
        EmbeddedPage(
            DocumentView::open(
                &mut Document::builder(LONG, ""),
                size(px(400.), px(300.)),
                window,
                cx,
            )
            .unwrap(),
        )
    });
    frame(cx);
    frame(cx);
    let page = cx.update(|_, cx| root.read(cx).0.clone());
    let before = cx.update(|_, cx| page.read(cx).document.builds_inside_draw());
    for width in [350., 450.] {
        cx.update(|_, cx| {
            page.update(cx, |page, cx| {
                page.resize(size(px(width), px(300.)), cx).unwrap()
            })
        });
        frame(cx);
        cx.update(|_, cx| assert_eq!(page.read(cx).document.builds_inside_draw(), before));
    }
}
