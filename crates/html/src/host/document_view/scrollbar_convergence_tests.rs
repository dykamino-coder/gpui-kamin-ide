//! Scrollbar convergence with nested grids and fractional textarea geometry.
use super::tests::frame;
use super::*;
use gpui::{TestAppContext, size};

#[gpui::test]
fn centered_grid_textarea_auto_scrollbars_converge(cx: &mut TestAppContext) {
    check_scrollbars(cx, "auto");
}

#[gpui::test]
fn centered_grid_textarea_visible_scrollbars_converge(cx: &mut TestAppContext) {
    check_scrollbars(cx, "visible");
}

#[gpui::test]
fn centered_grid_textarea_forced_scrollbars_converge(cx: &mut TestAppContext) {
    check_scrollbars(cx, "scroll");
}

fn check_scrollbars(cx: &mut TestAppContext, overflow: &str) {
    let html = format!(
        "<!doctype html><html><head><style>
        body{{font-family:monospace}}
        ul{{display:grid}}li{{display:grid;place-content:center;font-size:32px;font-weight:700}}
        textarea{{overflow:{overflow}}}
        </style></head><body><main><article><div><ul><li>
        <label>Fully customized</label><textarea rows=3></textarea>
        </li></ul></div></article></main></body></html>"
    );
    check_page(cx, &html, false);
}

#[gpui::test]
fn nested_ancestor_and_descendant_auto_scrollbars_converge(cx: &mut TestAppContext) {
    check_page(
        cx,
        "<!doctype html><style>
        body{font-family:monospace}
        .outer{width:320px;height:80px;overflow:auto}
        .inner{width:360px;height:160px;overflow:auto}
        .fill{width:500px;height:240px}
        ul{display:grid}li{display:grid;place-content:center;font-size:32px;font-weight:700}
        textarea{overflow:auto}
        </style><div class=outer><div class=inner><ul><li>
        <label>Fully customized</label><textarea rows=3></textarea>
        </li></ul><div class=fill></div></div></div>",
        true,
    );
}

fn check_page(cx: &mut TestAppContext, html: &str, nested: bool) {
    let (_one, _reader) = crate::why::test_reader();
    let budget = gpui::dirty_sources::trace::LayoutPassBudget::new(128);
    let mut app = TestAppContext::build_with_text_system(
        cx.dispatcher.clone(),
        None,
        std::sync::Arc::new(super::fractional_text_tests::FractionalTextSystem),
    );
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
    if nested {
        let journal = crate::why::take().journal;
        let both: std::collections::HashSet<_> = journal
            .causes
            .iter()
            .filter(|cause| {
                cause.code == "scroll_extent.layout_bars"
                    && cause.detail.contains("-> [true, true]")
            })
            .map(|cause| cause.detail.split(" bars ").next().unwrap())
            .collect();
        assert_eq!(
            both.len(),
            2,
            "both nested scrollports need both bars: {journal:?}"
        );
    }
    eprintln!(
        "nested:{nested}: {} native dependency retries",
        budget.retries()
    );
}
