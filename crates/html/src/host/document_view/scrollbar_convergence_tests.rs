//! Сходимость auto-полос при вложенном grid и дробной геометрии textarea.
use super::tests::frame;
use super::*;
use gpui::{TestAppContext, size};

#[gpui::test]
fn centered_grid_textarea_auto_scrollbars_converge(cx: &mut TestAppContext) {
    let (_one, _reader) = crate::why::test_reader();
    let budget = gpui::dirty_sources::trace::LayoutPassBudget::new(128);
    let mut app = TestAppContext::build_with_text_system(
        cx.dispatcher.clone(),
        None,
        std::sync::Arc::new(super::fractional_text_tests::FractionalTextSystem),
    );
    let html = "<!doctype html><html><head><style>
        body{font-family:monospace}
        ul{display:grid}li{display:grid;place-content:center;font-size:32px;font-weight:700}
        textarea{overflow:auto}
        </style></head><body><main><article><div><ul><li>
        <label>Fully customized</label><textarea rows=3></textarea>
        </li></ul></div></article></main></body></html>";
    let viewport = size(px(1446.4), px(804.));
    let (_, cx) = app.add_window_view(|window, cx| {
        let fonts = crate::text::metrics::FontContext::new(cx.text_system().clone());
        let document = Document::builder(html, "")
            .fonts(&fonts)
            .build_for_view(true)
            .unwrap();
        DocumentView::new(document, viewport, window, cx)
    });
    cx.simulate_scale_factor_change(1.25);
    cx.simulate_resize(viewport);
    frame(cx);
    frame(cx);
    assert!(budget.retries() <= 128);
}
