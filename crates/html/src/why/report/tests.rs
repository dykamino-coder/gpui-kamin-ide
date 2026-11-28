//! Owner-walk regressions run in ordinary workspace tests, without a profiler feature.

use super::*;
use crate::host::document_view::DocumentView;
use gpui::{
    Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext, Window, px, size,
};

struct Host(Entity<DocumentView>);

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0.clone()
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

#[gpui::test]
fn redraws_do_not_walk_owners_and_explicit_snapshots_follow_mutations(cx: &mut TestAppContext) {
    let (_one, reader) = crate::why::test_reader();
    drop(reader);
    let mut report = Report::new();
    let (host, cx) = cx.add_window_view(|window, cx| {
        Host(
            DocumentView::open(
                &mut Document::builder("<main>before</main>", ""),
                size(px(400.), px(300.)),
                window,
                cx,
            )
            .unwrap(),
        )
    });
    let page = cx.update(|_, cx| host.read(cx).0.clone());
    cx.update(|window, cx| {
        report.sample_memory("open", page.read(cx).document(), window, cx, None);
    });
    for _ in 0..3 {
        let watch = cx.update(|_, cx| report.watch(page.read(cx).document()));
        draw(cx);
        cx.update(|_, cx| {
            report
                .record(watch, page.read(cx).document(), false, json!({}))
                .unwrap();
        });
    }
    let before = report.to_json();
    assert_eq!(before["memory_samples"].as_array().unwrap().len(), 1);
    assert_eq!(before["frames"].as_array().unwrap().len(), 3);
    #[cfg(feature = "heap-report")]
    assert_eq!(before["owner_walks"], 1, "redraws walked owners");
    for frame in before["frames"].as_array().unwrap() {
        assert_eq!(frame["memory"]["sample_id"], 1);
        assert_eq!(frame["memory"]["sampled"], false);
        assert_eq!(frame["phases_ms"]["journal_memory"], 0.);
        assert!(frame["memory"].get("owners").is_none());
    }
    cx.update(|window, cx| {
        page.update(cx, |page, cx| {
            page.document_mut()
                .update(&format!("<main>{}</main>", "<p>new</p>".repeat(100)), "")
                .unwrap();
            cx.notify();
        });
        report.sample_memory("request", page.read(cx).document(), window, cx, None);
    });
    let watch = cx.update(|_, cx| report.watch(page.read(cx).document()));
    draw(cx);
    cx.update(|_, cx| {
        report
            .record(watch, page.read(cx).document(), false, json!({}))
            .unwrap();
    });
    let after = report.to_json();
    assert_eq!(after["memory_samples"].as_array().unwrap().len(), 2);
    assert_eq!(after["frames"][3]["memory"]["sample_id"], 2);
    #[cfg(feature = "heap-report")]
    {
        assert_eq!(after["owner_walks"], 2, "mutation redraw walked owners");
        assert_eq!(after["memory_samples"][1]["owner_walks"], 1);
        assert_ne!(
            before["memory_samples"][0]["owners"]["dom/nodes"],
            after["memory_samples"][1]["owners"]["dom/nodes"]
        );
    }
}
