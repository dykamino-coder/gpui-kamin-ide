//! Actual viewer chrome, fractional document embedding and native bounds callbacks.

use crate::{app::Browser, viewport};
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, point, px, size};
use kamin_html::{Document, host::document_view::DocumentView, metrics::FontContext};

fn open(cx: &mut TestAppContext) -> (Entity<Browser>, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        let fonts = FontContext::new(cx.text_system().clone());
        let mut view = Browser::new(None, fonts, window, cx);
        view.toggle_hud(window, cx);
        view.folder = Some("fixture".into());
        view.files = vec!["fixture/page.html".into()];
        view.current = view.files.first().cloned();
        view.preset_width = Some(1206);
        view
    });
    cx.simulate_scale_factor_change(1.25);
    cx.simulate_resize(size(px(1446.4), px(804.)));
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.resize(window, cx);
            view.page = Some(
                DocumentView::open(
                    &mut Document::builder(
                        "<style>body{margin:0}main{width:100%;height:728px;background:blue}</style><main></main>",
                        "",
                    ),
                    size(px(view.viewport.0), px(view.viewport.1)),
                    window,
                    cx,
                )
                .unwrap(),
            );
            cx.notify();
        });
    });
    draw(cx);
    draw(cx);
    (view, cx)
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn assert_document_box(cx: &mut VisualTestContext, width: f32) {
    cx.update(|window, _| {
        let scale = window.scale_factor();
        let boxes: Vec<_> = window
            .painted_quads()
            .iter()
            .map(|quad| quad.bounds)
            .filter(|bounds| {
                (bounds.origin.x.0 / scale - viewport::SIDEBAR_WIDTH).abs() < 0.01
                    && (bounds.origin.y.0 / scale - viewport::TOOLBAR_HEIGHT).abs() < 0.01
            })
            .collect();
        assert!(
            boxes.iter().any(|bounds| {
                (bounds.size.width.0 / scale - width).abs() < 0.01
                    && (bounds.size.height.0 / scale - 728.).abs() < 0.01
            }),
            "no document box of width {width} at the viewer's content origin: physical={boxes:?}, scale={scale}"
        );
    });
}

#[gpui::test]
fn mouse_moves_in_viewer_do_not_build_or_move_the_document(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    cx.simulate_mouse_move(point(px(700.), px(400.)), None, Modifiers::default());
    draw(cx);
    let page = cx.update(|_, cx| view.read(cx).page.clone().unwrap());
    let before = cx.update(|_, cx| page.read(cx).document().frame_counters());
    for step in 0..32 {
        cx.simulate_mouse_move(
            point(px(700. + (step % 7) as f32), px(400.)),
            None,
            Modifiers::default(),
        );
        draw(cx);
        assert_document_box(cx, 1206.4);
    }
    // Host hover styles must not change the embedded content box either.
    for position in [
        point(px(80.), px(90.)),
        point(px(30.), px(20.)),
        point(px(700.), px(400.)),
    ] {
        cx.simulate_mouse_move(position, None, Modifiers::default());
        draw(cx);
        assert_document_box(cx, 1206.4);
    }
    cx.update(|_, cx| {
        let (builds, _, inside) = page
            .read(cx)
            .document()
            .frame_counters()
            .since(&before)
            .unwrap();
        assert_eq!((builds, inside), (0, 0));
    });
}

#[gpui::test]
fn native_resize_releases_scenario_preset_and_reaches_document(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let page = cx.update(|_, cx| {
        assert_eq!(view.read(cx).viewport, (1206., 728.));
        view.read(cx).page.clone().unwrap()
    });
    let before = cx.update(|_, cx| page.read(cx).document().frame_counters());
    // 1808 -> 1728 physical client pixels, through the platform resize callback.
    cx.simulate_resize(size(px(1382.4), px(804.)));
    draw(cx);
    cx.update(|_, cx| {
        assert!((view.read(cx).viewport.0 - 1142.4).abs() < 0.01);
        assert!(
            page.read(cx)
                .document()
                .frame_counters()
                .since(&before)
                .unwrap()
                .0
                > 0
        );
    });
    assert_document_box(cx, 1142.4);
}
