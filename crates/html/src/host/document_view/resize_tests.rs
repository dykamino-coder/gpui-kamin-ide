//! Resize and embedding checks for the document view: media environment, prepared reuse.

use super::tests::{LONG, frame, open};
use super::*;
use gpui::{Entity, TestAppContext, VisualTestContext, size};

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
