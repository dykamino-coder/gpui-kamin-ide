//! Глубокое содержимое проходит построение, layout и paint без усечения.

use super::*;
use crate::document::RevisionStatus;

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
fn reflow_keeps_content_beyond_the_former_layout_limit() {
    regression::on_eight_mib(|| {
        let document = Document::builder(&(markup(1500) + "<p>tail</p>"), "")
            .build()
            .unwrap();
        assert_eq!(document.report().status, RevisionStatus::Complete);
        draw(&document);
        assert_eq!(document.report().status, RevisionStatus::Complete);
        assert!(document.traversal_limit().is_none());
        assert_eq!(source_text(&document), "leaftail");
        let mut text = String::new();
        crate::dom::text_content::gather_text(document.nodes(), &mut text);
        assert_eq!(text, "leaftail");
    });
}

#[test]
fn repeated_deep_frames_do_not_report_depth_degradation() {
    regression::on_eight_mib(|| {
        let document = Document::builder(&markup(1500), "").build().unwrap();
        for _ in 0..2 {
            draw(&document);
            assert!(document.take_diagnostics().is_empty());
            assert_eq!(document.report().status, RevisionStatus::Complete);
        }
    });
}

#[test]
fn beyond_the_former_box_limit_paints_the_leaf_and_following_sibling() {
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
            draw(&document);
            assert_eq!(document.report().status, RevisionStatus::Complete);
            assert!(document.traversal_limit().is_none());
            assert_eq!(source_text(&document), "leaftail");
            let mut text = String::new();
            crate::dom::text_content::gather_text(document.nodes(), &mut text);
            assert_eq!(text, "leaftail");
        }
    });
}

/// Регрессия #618 / INC-2026-0084: полный кадр float на прежнем стеке 7,5 МиБ.
#[test]
fn nested_floats_up_to_box_limit_lay_out_on_seven_and_a_half_mib() {
    regression::on_stack(15 * 512 * 1024, || {
        let source = format!(
            "{}leaf{}<p>tail</p>",
            "<div style='float:left'>".repeat(2053),
            "</div>".repeat(2053)
        );
        let document = Document::builder(&source, "").build().unwrap();
        draw(&document);
        assert_eq!(document.report().status, RevisionStatus::Complete);
        assert!(document.traversal_limit().is_none());
        assert_eq!(source_text(&document), "leaftail");
    });
}

#[test]
fn twenty_thousand_levels_complete_all_frame_phases_on_a_small_stack() {
    regression::on_stack(512 * 1024, || {
        let document = Document::builder(&markup(20_000), "").build().unwrap();
        draw(&document);
        assert_eq!(document.report().status, RevisionStatus::Complete);
        assert!(document.take_diagnostics().is_empty());
        let mut text = String::new();
        crate::dom::text_content::gather_text(document.nodes(), &mut text);
        assert_eq!(text, "leaf");
    });
}

/// Source URLs exercise resource resolution before the retained viewport path.
#[test]
fn sourced_twenty_thousand_levels_complete_a_retained_frame_on_a_small_stack() {
    regression::on_stack(512 * 1024, || {
        let document = Document::builder(&(markup(20_000) + "<p>tail</p>"), "")
            .base_uri("file:///tests/deep.html")
            .build()
            .unwrap();
        let mut cx = gpui::TestAppContext::single();
        cx.add_empty_window().draw(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(100.), gpui::px(100.)),
            |_, _| FrameProbe(document.render_viewport(&options()), "paint".into()),
        );
        assert_eq!(document.report().status, RevisionStatus::Complete);
        assert!(document.take_diagnostics().is_empty());
        assert_eq!(source_text(&document), "leaftail");
        let mut text = String::new();
        crate::dom::text_content::gather_text(document.nodes(), &mut text);
        assert_eq!(text, "leaftail");
    });
}
