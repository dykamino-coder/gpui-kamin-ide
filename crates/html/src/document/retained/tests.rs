//! Сохраняемый кадр: неизменный документ повторяется, любой изменённый вход
//! (опции хоста, ввод, изменивший состояние элементов, живая анимация, новое дерево)
//! собирает кадр заново; ввод без изменений — `input_replay_tests`.

use super::box_repaint_tests::frames;
use crate::{Document, RenderOpts};
use gpui::{Context, IntoElement, Render, TestAppContext, VisualTestContext, Window};

pub(super) struct View {
    pub(super) doc: Document,
    pub(super) viewport: (f32, f32),
    /// The document step before every draw (`Document::update_rendering`).
    step: Option<gpui::Subscription>,
}

impl View {
    fn opts(&self) -> RenderOpts {
        let mut opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: self.viewport,
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.deferred.freeze_scrollbars = true;
        opts
    }
}

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.doc.set_viewport(self.viewport).unwrap();
        self.doc.render_viewport(&self.opts())
    }
}

pub(super) fn open<'a>(
    cx: &'a mut TestAppContext,
    html: &str,
) -> (gpui::Entity<View>, &'a mut VisualTestContext) {
    open_document(cx, Document::builder(html, "").build().unwrap())
}

/// The host with the document step over an already built document.
pub(super) fn open_document(
    cx: &mut TestAppContext,
    doc: Document,
) -> (gpui::Entity<View>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_, _| View {
        doc,
        viewport: (400., 300.),
        step: None,
    });
    let stepped = view.clone();
    let parent = gpui::ElementPlace::of_view(view.entity_id());
    cx.update(|window, app| {
        let step = window.on_update_rendering(move |window, app| {
            stepped.update(app, |view, cx| {
                view.doc.set_viewport(view.viewport).unwrap();
                view.doc
                    .update_rendering(&view.opts(), &parent, window, cx, |_| {})
            })
        });
        view.update(app, |view, _| view.step = Some(step));
    });
    (view, cx)
}

/// The host without the document step: every frame builds inside the draw
/// (the frame path before `Document::update_rendering`).
pub(super) fn open_without_step<'a>(
    cx: &'a mut TestAppContext,
    html: &str,
) -> (gpui::Entity<View>, &'a mut VisualTestContext) {
    let doc = Document::builder(html, "").build().unwrap();
    cx.add_window_view(|_, _| View {
        doc,
        viewport: (400., 300.),
        step: None,
    })
}

pub(super) fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

const PAGE: &str = "<p style='margin:20px;color:red'>hello <b>world</b></p><div style='width:50px;height:50px;background:blue'></div>";

#[gpui::test]
fn unchanged_frame_is_replayed_without_a_build(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, PAGE);
    let (builds, _) = frames(&view, cx);
    frame(cx);
    frame(cx);
    let (after, replays) = frames(&view, cx);
    assert_eq!(after, builds, "unchanged frames rebuilt the document");
    assert!(
        replays >= 2,
        "unchanged frames were not replayed: {replays}"
    );
}

#[gpui::test]
fn viewport_change_rebuilds(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, PAGE);
    frame(cx);
    let (builds, replays) = frames(&view, cx);
    cx.update(|_, cx| view.update(cx, |v, _| v.viewport = (500., 300.)));
    frame(cx);
    assert_eq!(frames(&view, cx), (builds + 1, replays));
    frame(cx);
    assert_eq!(frames(&view, cx), (builds + 1, replays + 1));
}

#[gpui::test]
fn live_animation_is_never_replayed(cx: &mut TestAppContext) {
    let html = "<style>@keyframes k{from{width:10px}to{width:90px}}</style>\
                <div style='height:20px;background:red;animation:k 1s infinite'></div>";
    let (view, cx) = open(cx, html);
    frame(cx);
    frame(cx);
    frame(cx);
    assert_eq!(frames(&view, cx).1, 0, "a live animation was replayed");
}

#[gpui::test]
fn updated_markup_rebuilds(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, PAGE);
    frame(cx);
    frame(cx);
    cx.update(|_, cx| {
        view.update(cx, |v, _| {
            v.doc.update("<p>changed</p>", "").unwrap();
        })
    });
    frame(cx);
    assert_eq!(
        frames(&view, cx),
        (1, 0),
        "a new tree replayed the old frame"
    );
}

/// A rebuilt frame can reuse line splits from the previous frame.
#[gpui::test]
fn a_rebuilt_frame_reuses_the_previous_frames_line_splits(cx: &mut TestAppContext) {
    let spans: String = (0..60)
        .map(|i| format!("<span>word{i} alpha beta </span>"))
        .collect();
    let a = format!("<p style='width:300px'>{spans}</p><p>second</p>");
    let b = a.replacen("<span>word40", "<span data-edit=1>word40", 1);
    let (view, cx) = open(cx, &a);
    frame(cx);
    let misses = |view: &gpui::Entity<View>, cx: &mut VisualTestContext| {
        cx.update(|_, cx| {
            view.read(cx)
                .doc
                .state
                .paragraph
                .paragraphs
                .split_misses
                .get()
        })
    };
    let first = misses(&view, cx);
    assert!(first > 0, "the first frame splits its paragraphs");
    for html in [&b, &a] {
        cx.update(|_, cx| {
            view.update(cx, |v, _| {
                v.doc.update(html, "").unwrap();
            })
        });
        frame(cx);
        assert_eq!(
            misses(&view, cx),
            first,
            "an unchanged paragraph was split again"
        );
    }
    let (builds, _) = frames(&view, cx);
    assert!(builds >= 1);
}

/// A revision whose records change no box (a class on `html` that no sheet
/// names) keeps the recorded frame: the next frame replays it, nothing is built.
#[gpui::test]
fn a_clean_revision_replays_the_recorded_frame(cx: &mut TestAppContext) {
    let page =
        "<!doctype html><html><head></head><body><p style='color:red'>hello</p></body></html>";
    let (view, cx) = open(cx, page);
    frame(cx);
    frame(cx);
    let (builds, replays) = frames(&view, cx);
    cx.update(|_, cx| {
        view.update(cx, |v, _| {
            let dom = v.doc.source_dom();
            let html = dom.children.borrow().last().cloned().unwrap();
            let record = crate::Mutation::Attribute {
                element: html,
                name: html5ever::QualName::new(None, html5ever::ns!(), "class".into()),
                value: Some("alt".into()),
            };
            assert!(v.doc.apply(&[record], "").unwrap().changed());
        })
    });
    frame(cx);
    assert_eq!(
        frames(&view, cx),
        (builds, replays + 1),
        "a clean revision rebuilt its frame"
    );
}
