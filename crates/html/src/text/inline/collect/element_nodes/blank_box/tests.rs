//! Пустая inline-коробка: исходный и подготовленный текстовые входы совпадают.

use super::*;
use crate::layout::table::anon::anon_element;

#[test]
fn blank_text_facts_preserve_spaces_background_and_nonbreaking_spaces() {
    for (text, empty, collapsible) in [
        ("", true, true),
        (" \t\n\r", false, true),
        ("\u{a0}", false, false),
        (" x ", false, false),
        ("\u{200b}", false, false),
    ] {
        let e = anon_element("span", vec![Node::Text(text.into())]);
        for keep in [false, true] {
            for painted in [false, true] {
                let expected = empty || (!keep && !painted && collapsible);
                assert_eq!(text_is_blank(&e, keep, painted), expected);
                e.children.publish_box_summary(crate::dom::BoxTreeSummary {
                    text: crate::dom::BoxTextFacts::text(text),
                    ..Default::default()
                });
                assert_eq!(text_is_blank(&e, keep, painted), expected);
                e.children.dirty_intrinsic_sizes();
            }
        }
    }
}

#[test]
fn nested_boxes_are_not_blank_even_without_text() {
    let e = anon_element("span", vec![Node::Element(anon_element("br", vec![]))]);
    assert!(!text_is_blank(&e, false, false));
    e.children.publish_box_summary(crate::dom::BoxTreeSummary {
        text: crate::dom::BoxTextFacts {
            elements: true,
            ..Default::default()
        },
        ..Default::default()
    });
    assert!(!text_is_blank(&e, false, false));
}

#[cfg(feature = "work-counters")]
#[test]
fn blank_boxes_do_not_query_or_visit_descendants() {
    use crate::work::{Counter, Session};
    let e = anon_element(
        "span",
        vec![Node::Element(anon_element(
            "span",
            vec![Node::Text("content".into())],
        ))],
    );
    let style = BoxStyle::default();
    let cx = crate::layout::cx::LayoutCx::default();
    let mut pieces = Vec::new();
    for prepared in [false, true] {
        if prepared {
            e.children.publish_box_summary(crate::dom::BoxTreeSummary {
                text: crate::dom::BoxTextFacts {
                    elements: true,
                    in_flow: true,
                    ..Default::default()
                },
                ..Default::default()
            });
        }
        let session = Session::begin();
        for _ in 0..1024 {
            assert!(!blank_box(
                &e,
                &style,
                &style,
                false,
                0.0,
                0.0,
                &mut pieces,
                &cx
            ));
        }
        let work = session.finish();
        for counter in [Counter::SubtreeQueries, Counter::SubtreeNodesVisited] {
            assert_eq!(
                work.0
                    .iter()
                    .map(|phase| phase[counter as usize])
                    .sum::<u64>(),
                0
            );
        }
        assert!(pieces.is_empty());
    }
}
