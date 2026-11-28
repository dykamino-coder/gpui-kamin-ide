//! Host-only frames carry the complete document scene without entering the engine.

use super::tests::frame;
use super::*;
use gpui::{AnyView, TestAppContext, VisualTestContext, size};

struct Host {
    page: Entity<DocumentView>,
    pulse: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(400.))
            .h(px(300.))
            .child(AnyView::from(self.page.clone()))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(if self.pulse { 80. } else { 81. }))
                    .h(px(60.))
                    .bg(gpui::rgb(0x00ff00)),
            )
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn assert_host_on_top(cx: &mut VisualTestContext, width: f32) {
    cx.update(|window, _| {
        let scale = window.scale_factor();
        let quads = window.painted_quads();
        let sequence = window.painted_sequence();
        let order = |width: f32, height: f32| {
            let quad = quads
                .iter()
                .find(|q| {
                    (q.bounds.size.width.0 / scale - width).abs() < 0.1
                        && (q.bounds.size.height.0 / scale - height).abs() < 0.1
                })
                .expect("expected painted box");
            let fill = format!("background: {:?},", quad.background);
            sequence
                .iter()
                .position(|(p, _)| p.starts_with("quad ") && p.contains(&fill))
                .unwrap()
        };
        assert!(
            order(width, 60.) > order(50., 50.) && order(width, 60.) > order(40., 40.),
            "document escaped its stacking root"
        );
    });
}

#[gpui::test]
fn host_only_frames_skip_every_engine_entry_and_keep_document_layers(cx: &mut TestAppContext) {
    let (_one, _reader) = crate::why::test_reader();
    for position in ["fixed", "relative", "sticky", "absolute"] {
        let mut app = cx.clone();
        let html = format!(
            "<style>body{{margin:0}}header{{position:{position};top:0;left:0;\
            width:50px;height:50px;background:red;z-index:2147483647}}\
            main{{height:1800px;background:blue}}</style><header>\
            <i style='position:fixed;top:0;left:0;width:40px;height:40px;\
            background:yellow;z-index:2147483647'></i></header><main></main>"
        );
        let (host, cx) = app.add_window_view(move |window, cx| {
            let page = DocumentView::open(
                &mut Document::builder(&html, ""),
                size(px(400.), px(300.)),
                window,
                cx,
            )
            .unwrap();
            Host { page, pulse: false }
        });
        frame(cx);
        frame(cx);
        let page = cx.update(|_, cx| host.read(cx).page.clone());
        cx.update(|_, cx| page.read(cx).document.take_root_phases());
        assert_host_on_top(cx, 81.);
        for pulse in [true, false, true] {
            cx.update(|_, cx| {
                host.update(cx, |host, cx| {
                    host.pulse = pulse;
                    cx.notify();
                })
            });
            draw(cx);
            let sample = cx.update(|_, cx| page.read(cx).document.take_root_phases());
            assert_eq!(sample["work_calls"]["total"], 0, "{sample}");
            assert!(sample["attempts"].as_array().unwrap().is_empty());
            assert_host_on_top(cx, if pulse { 80. } else { 81. });
        }
        // A real mutation still invalidates the cached scene after host-only frames.
        cx.update(|_, cx| {
            page.update(cx, |page, cx| {
                page.document
                    .update(
                        "<main style='width:70px;height:90px;background:red'></main>",
                        "",
                    )
                    .unwrap();
                cx.notify();
            })
        });
        draw(cx);
        let sample = cx.update(|_, cx| page.read(cx).document.take_root_phases());
        assert!(sample["work_calls"]["total"].as_u64().unwrap() > 0);
        assert!(!sample["attempts"].as_array().unwrap().is_empty());
        cx.update(|window, _| {
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .any(|q| q.bounds.size.width.0 / window.scale_factor() == 70.
                        && q.bounds.size.height.0 / window.scale_factor() == 90.)
            )
        });
    }
}

#[gpui::test]
fn root_document_view_also_skips_an_unchanged_window_frame(cx: &mut TestAppContext) {
    let (_one, _reader) = crate::why::test_reader();
    let html = super::fixed_tests::rows(&super::fixed_tests::fixed_box());
    let (page, cx) = super::tests::open(cx, &html);
    cx.update(|_, cx| {
        page.read(cx).document.take_root_phases();
    });
    draw(cx);
    cx.update(|_, cx| {
        assert_eq!(
            page.read(cx).document.take_root_phases()["work_calls"]["total"],
            0
        )
    });
    cx.update(|_, cx| {
        page.update(cx, |page, cx| {
            page.resize(size(px(250.), px(200.)), cx).unwrap()
        })
    });
    draw(cx);
    cx.update(|_, cx| {
        let page = page.read(cx);
        let root = page.document.state_rc().root_scroll.clone().unwrap();
        assert_eq!(root.handle.bounds().size, size(px(250.), px(200.)));
        assert!(
            page.document.take_root_phases()["work_calls"]["total"]
                .as_u64()
                .unwrap()
                > 0
        );
    });
    draw(cx);
    cx.update(|_, cx| {
        assert_eq!(
            page.read(cx).document.take_root_phases()["work_calls"]["total"],
            0
        )
    });
    let fixed = super::fixed_tests::boxes(cx);
    super::fixed_tests::wheel(cx);
    draw(cx);
    assert!(super::fixed_tests::root(&page, cx).offset().y < px(0.));
    assert_eq!(super::fixed_tests::boxes(cx), fixed);
    cx.update(|_, cx| {
        assert!(
            page.read(cx).document.take_root_phases()["work_calls"]["total"]
                .as_u64()
                .unwrap()
                > 0
        )
    });
}

mod sibling_tests;
