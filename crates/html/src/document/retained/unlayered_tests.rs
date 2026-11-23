//! An offscreen edit without a root layer preserves the frame until scrolling.
//! Unknown or visible damage requires a new frame.

use crate::document::Document;
use crate::host::document_view::DocumentView;
use gpui::{Entity, IntoElement, TestAppContext, VisualTestContext, point, px, size};

struct Page(Entity<DocumentView>);

impl gpui::Render for Page {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        self.0.clone()
    }
}

fn page(edit: &str, at: usize, css: &str, list: bool) -> String {
    let spans: String = (0..90)
        .map(|i| {
            let edit = if i == at { edit } else { "" };
            format!(
                "<span id=s{i}{edit}>lorem ipsum dolor sit amet consectetur adipiscing elit </span>"
            )
        })
        .collect();
    let content = if list {
        format!("<ol><li>{spans}</li></ol>")
    } else {
        format!("<p>{spans}</p>")
    };
    format!(
        "<!doctype html><html><head><style>html,body{{margin:0;background:white}}\
         ol{{margin:0;padding:0}}\
         p,li{{margin:0;width:380px;font:16px/20px sans-serif;text-align:justify}}\
         aside{{opacity:.5;width:10px;height:10px;background:blue}}\
         [data-edit]{{background:red}}{css}</style></head><body><aside></aside>{content}</body></html>"
    )
}

fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn open<'a>(
    cx: &'a mut TestAppContext,
    html: &str,
) -> (Entity<DocumentView>, &'a mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        // Disable layers explicitly: ordinary opacity can still record a layer.
        window.set_scene_layers(false);
        Page(
            DocumentView::open(
                &mut Document::builder(html, ""),
                size(px(400.), px(300.)),
                window,
                cx,
            )
            .unwrap(),
        )
    });
    cx.simulate_resize(size(px(400.), px(300.)));
    frame(cx);
    frame(cx);
    let page = cx.update(|_, cx| view.read(cx).0.clone());
    (page, cx)
}

fn update(view: &Entity<DocumentView>, cx: &mut VisualTestContext, html: &str) {
    cx.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.document_mut().update(html, "").unwrap();
            cx.notify();
        })
    });
    frame(cx);
}

fn stats(view: &Entity<DocumentView>, cx: &mut VisualTestContext) -> ((u64, u64), usize) {
    cx.update(|_, cx| {
        let doc = view.read(cx).document();
        assert!(
            doc.reuse.scene_layer().is_none(),
            "fixture acquired a root layer"
        );
        (doc.retained_frames(), doc.reuse.deferred_frames())
    })
}

fn root(view: &Entity<DocumentView>, cx: &mut VisualTestContext) -> gpui::ScrollHandle {
    cx.update(|_, cx| {
        view.read(cx)
            .document()
            .state
            .root_scroll
            .as_ref()
            .unwrap()
            .handle
            .clone()
    })
}

/// Replay translation and a fresh build produce the same bounds.
fn quads(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|window, _| {
        let viewport = gpui::Bounds {
            origin: gpui::point(gpui::ScaledPixels(0.), gpui::ScaledPixels(0.)),
            size: window.viewport_size().scale(window.scale_factor()),
        };
        let mut keys: Vec<_> = window
            .painted_quads()
            .into_iter()
            .filter_map(|mut q| {
                q.order = 0;
                let t = &mut q.transformation;
                assert_eq!(t.rotation_scale, [[1., 0.], [0., 1.]]);
                q.bounds.origin.x.0 += t.translation[0];
                q.bounds.origin.y.0 += t.translation[1];
                t.translation = [0., 0.];
                let visible = q
                    .bounds
                    .intersect(&q.content_mask.bounds)
                    .intersect(&viewport);
                (!visible.is_empty()).then(|| format!("{q:?}"))
            })
            .collect();
        keys.sort();
        keys
    })
}

fn fresh(cx: &mut TestAppContext, html: &str, y: f32) -> Vec<String> {
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, html);
    root(&view, vcx).set_offset(point(px(0.), px(y)));
    update(&view, vcx, &format!("{html}<!-- fresh -->"));
    frame(vcx);
    assert_eq!(root(&view, vcx).offset().y, px(y));
    quads(vcx)
}

#[gpui::test]
fn deferred_inline_paint_without_a_layer_appears_when_scrolled_to(cx: &mut TestAppContext) {
    deferred_paint(cx, false);
}

#[gpui::test]
fn deferred_list_item_paint_without_a_layer_appears_when_scrolled_to(cx: &mut TestAppContext) {
    deferred_paint(cx, true);
}

fn deferred_paint(cx: &mut TestAppContext, list: bool) {
    let old = page("", 20, "", list);
    let new = page(" data-edit", 20, "", list);
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &old);
    let before = stats(&view, vcx).0;
    update(&view, vcx, &new);
    let (after, waiting) = stats(&view, vcx);
    assert_eq!(after.0, before.0, "offscreen edit built a frame");
    assert!(after.1 > before.1, "offscreen edit did not replay");
    assert!(waiting > 0, "no deferred edit");
    assert_eq!(quads(vcx), fresh(cx, &new, 0.));
    // Several small scroll steps can remain inside the recorded interest area.
    let mut saw_edit = false;
    for _ in 0..8 {
        vcx.simulate_event(gpui::ScrollWheelEvent {
            position: point(px(200.), px(20.)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-120.))),
            modifiers: Default::default(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        frame(vcx);
        let y = f32::from(root(&view, vcx).offset().y);
        assert!(y < 0.);
        let expected = fresh(cx, &new, y);
        saw_edit |= expected != fresh(cx, &old, y);
        assert_eq!(quads(vcx), expected, "scroll offset {y}");
    }
    assert!(saw_edit, "edit never became visible");
}

#[gpui::test]
fn visible_inline_paint_without_a_layer_rebuilds(cx: &mut TestAppContext) {
    visible_paint(cx, false);
}

#[gpui::test]
fn visible_list_item_paint_without_a_layer_rebuilds(cx: &mut TestAppContext) {
    visible_paint(cx, true);
}

fn visible_paint(cx: &mut TestAppContext, list: bool) {
    let old = page("", 1, "", list);
    let new = page(" data-edit", 1, "", list);
    let mut app = cx.clone();
    let (view, vcx) = open(&mut app, &old);
    let before = vcx.update(|_, cx| view.read(cx).document().retained.clone());
    update(&view, vcx, &new);
    let after = vcx.update(|_, cx| view.read(cx).document().retained.clone());
    // A new Slot resets its counters, so compare its identity.
    assert!(!std::rc::Rc::ptr_eq(&before, &after));
    assert!(stats(&view, vcx).0.0 > 0);
    assert_eq!(quads(vcx), fresh(cx, &new, 0.));
}
