//! Сохраняемый кадр: неизменный документ повторяется, любой изменённый вход
//! (опции хоста, ввод окна, живая анимация, новое дерево) собирает кадр заново.

use crate::{Document, RenderOpts};
use gpui::{
    Context, IntoElement, Modifiers, Render, TestAppContext, VisualTestContext, Window, point, px,
};

struct View {
    doc: Document,
    viewport: (f32, f32),
}

impl Render for View {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut opts = RenderOpts {
            text: gpui::TextStyle::default(),
            viewport: self.viewport,
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        opts.cx.deferred.freeze_scrollbars = true;
        self.doc.render_viewport(&opts)
    }
}

fn open<'a>(
    cx: &'a mut TestAppContext,
    html: &str,
) -> (gpui::Entity<View>, &'a mut VisualTestContext) {
    let doc = Document::builder(html, "").build().unwrap();
    cx.add_window_view(|_, _| View {
        doc,
        viewport: (400., 300.),
    })
}

fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn frames(view: &gpui::Entity<View>, cx: &mut VisualTestContext) -> (u64, u64) {
    cx.update(|_, cx| view.read(cx).doc.retained_frames())
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
fn window_input_rebuilds(cx: &mut TestAppContext) {
    let (view, cx) = open(cx, PAGE);
    frame(cx);
    let (builds, replays) = frames(&view, cx);
    cx.simulate_mouse_move(point(px(30.), px(30.)), None, Modifiers::default());
    frame(cx);
    let (after, again) = frames(&view, cx);
    assert!(after > builds, "input did not rebuild the frame");
    assert_eq!(again, replays);
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

#[gpui::test]
fn a_group_painted_by_a_deferred_draw_is_never_replayed(cx: &mut TestAppContext) {
    let page = "<div style='position:relative;z-index:1;width:60px;height:60px;background:lime'>\
        <div style='mix-blend-mode:multiply;width:30px;height:30px;background:red'></div></div>";
    let (view, cx) = open(cx, page);
    for _ in 0..4 {
        frame(cx);
    }
    let (builds, replays) = frames(&view, cx);
    assert_eq!(replays, 0, "a deferred composition group was replayed");
    assert!(
        builds >= 4,
        "frames with a deferred group were not rebuilt: {builds}"
    );
}

/// A rebuilt frame (here after a revision update) finds the line splits of the
/// previous frame: the measurement cache rotates once per frame, not once per
/// `begin_frame` and once more per tree build.
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
