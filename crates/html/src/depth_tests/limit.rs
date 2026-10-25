//! Reflow и построение боксов имеют разные пределы; исходный DOM не режется.

use super::*;
use crate::document::{RevisionStatus, TraversalKind};

fn draw(document: &Document) {
    let mut cx = gpui::TestAppContext::single();
    let boxes = document.render(&options());
    cx.add_empty_window().draw(
        gpui::point(gpui::px(0.), gpui::px(0.)),
        gpui::size(gpui::px(100.), gpui::px(100.)),
        |_, _| {
            FrameProbe(
                gpui::div().children(boxes).into_any_element(),
                "paint".into(),
            )
        },
    );
}

fn source_text(document: &Document) -> String {
    crate::dom::source_walk::descendants(&document.source_dom(), true)
        .filter_map(|node| {
            if let markup5ever_rcdom::NodeData::Text { contents } = &node.data {
                Some(contents.borrow().to_string())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn reflow_reports_its_limit_after_building_the_deeper_boxes() {
    regression::on_eight_mib(|| {
        let document = Document::builder(&(markup(1500) + "<p>tail</p>"), "")
            .build()
            .unwrap();
        assert_eq!(document.report().status, RevisionStatus::Complete);
        draw(&document);
        let report = document.report();
        assert_eq!(report.status, RevisionStatus::Degraded);
        let limit = report.degradation.unwrap();
        assert_eq!(limit.traversal, TraversalKind::Layout);
        assert_eq!(limit.limit, 1026);
        assert_eq!(limit.attempted, 1027);
        assert_eq!(source_text(&document), "leaftail");
        let mut text = String::new();
        crate::dom::text_content::gather_text(document.nodes(), &mut text);
        assert_eq!(text, "leaftail");
    });
}

#[test]
fn beyond_box_limit_paints_the_remainder_and_keeps_the_whole_dom() {
    regression::on_eight_mib(|| {
        for source in [
            markup(4096),
            format!(
                "{}leaf{}",
                "<div style='float:left'>".repeat(2053),
                "</div>".repeat(2053)
            ),
        ] {
            let document = Document::builder(&(source + "<p>tail</p>"), "")
                .build()
                .unwrap();
            let before = document.report().degradation.unwrap();
            assert_eq!(before.traversal, TraversalKind::BoxBuild);
            assert_eq!(before.limit, 2052);
            draw(&document);
            assert_eq!(document.report().degradation, Some(before));
            assert_eq!(source_text(&document), "leaftail");
            let mut text = String::new();
            crate::dom::text_content::gather_text(document.nodes(), &mut text);
            assert_eq!(text, "tail");
        }
    });
}
