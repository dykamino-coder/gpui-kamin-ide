//! Retained normalization uses CSS Text inputs and feeds the existing paragraph adapter.

use super::*;
use crate::document::Document;
use crate::dom::Element;
use crate::layout::cx::{BuildDoc, LayoutCx};
use crate::style::computed::TextTransform;
use crate::style::values::value::Len;
use crate::text::inline::{self, Piece};

pub(super) fn paragraph(nodes: &[Node]) -> &Element {
    let mut pending = vec![nodes.iter()];
    while let Some(siblings) = pending.last_mut() {
        match siblings.next() {
            Some(Node::Element(e)) if e.tag == "p" => return e,
            Some(Node::Element(e)) => pending.push(e.children.iter()),
            Some(Node::Text(_)) => {}
            None => {
                pending.pop();
            }
        }
    }
    panic!("paragraph");
}

fn strings(pieces: Vec<Piece>) -> Vec<String> {
    pieces
        .into_iter()
        .map(|p| match p {
            Piece::Text { text, .. } => text,
            _ => panic!("text-only flow"),
        })
        .collect()
}

#[test]
fn normalized_runs_feed_the_adapter_and_survive_geometry_and_font_changes() {
    let doc = Document::builder(
        "<p style='text-transform:capitalize'>hello <![CDATA[ignored]]>world&#13; next</p>",
        "",
    )
    .build()
    .unwrap();
    let p = paragraph(&doc.nodes);
    let style = BoxStyle::shared(p.style.used().style);
    let cx = LayoutCx {
        values: crate::layout::cx::ValueContext {
            doc: BuildDoc::of(doc.state.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    doc.state
        .boxes
        .borrow_mut()
        .finish(&doc.nodes, &BoxStyle::default(), false);
    let first = doc
        .state
        .boxes
        .borrow()
        .normalized(&p.children, &style)
        .unwrap();
    let canonical = strings(inline::collect(
        &p.children,
        &style,
        &mut |_, _| None,
        &LayoutCx::default(),
    ));
    let retained = strings(inline::collect(
        &p.children,
        &style,
        &mut |_, _| panic!("no atoms"),
        &cx,
    ));
    assert_eq!(retained, canonical);
    assert_eq!(retained.concat(), "Hello World Next");
    let mut resized = style.clone();
    resized.reset_box_layout.width = Some(Len::Px(123.0));
    resized.inh.font.font_size = Some(Len::Px(28.0));
    let after = doc
        .state
        .boxes
        .borrow()
        .normalized(&p.children, &resized)
        .unwrap();
    assert!(first.same(&after));
    doc.state
        .boxes
        .borrow_mut()
        .finish(&doc.nodes, &BoxStyle::default(), false);
    let again = doc
        .state
        .boxes
        .borrow()
        .normalized(&p.children, &style)
        .unwrap();
    assert!(first.same(&again));
    resized.inh.text_flow.text_transform = Some(TextTransform::Upper);
    assert!(
        doc.state
            .boxes
            .borrow()
            .normalized(&p.children, &resized)
            .is_none()
    );
    let fallback = strings(inline::collect(
        &p.children,
        &resized,
        &mut |_, _| None,
        &cx,
    ));
    assert_eq!(fallback.concat(), "HELLO WORLD NEXT");
}

#[test]
fn a_style_commit_replaces_normalization_but_keeps_box_identity() {
    let mut doc = Document::builder("<p>hello  world</p>", "p {text-transform:uppercase}")
        .build()
        .unwrap();
    doc.state
        .boxes
        .borrow_mut()
        .finish(&doc.nodes, &BoxStyle::default(), false);
    let (id, text) = {
        let tree = doc.state.boxes.borrow();
        let p = paragraph(&doc.nodes);
        (
            tree.flow(&p.children).unwrap(),
            tree.normalized(&p.children, &p.style.used()).unwrap(),
        )
    };
    assert_eq!(text.text(), "HELLO WORLD");
    doc.update("<p>hello  world</p>", "p {text-transform:lowercase}")
        .unwrap();
    doc.state
        .boxes
        .borrow_mut()
        .finish(&doc.nodes, &BoxStyle::default(), false);
    let tree = doc.state.boxes.borrow();
    let p = paragraph(&doc.nodes);
    assert_eq!(tree.flow(&p.children).unwrap(), id);
    let next = tree.normalized(&p.children, &p.style.used()).unwrap();
    assert_eq!(next.text(), "hello world");
    assert!(!text.same(&next));
    assert_eq!(text.text(), "HELLO WORLD");
}

#[test]
fn mixed_flows_are_not_mistaken_for_flat_text() {
    let doc = Document::builder("<p>a<b>b</b><img>c</p>", "")
        .build()
        .unwrap();
    doc.state
        .boxes
        .borrow_mut()
        .finish(&doc.nodes, &BoxStyle::default(), false);
    let p = paragraph(&doc.nodes);
    assert!(
        doc.state
            .boxes
            .borrow()
            .normalized(&p.children, &p.style.used())
            .is_none()
    );
}

#[test]
fn an_unread_attribute_rebinds_the_adapter_without_replacing_runs() {
    let mut doc = Document::builder("<p class=a>hello world</p>", "")
        .build()
        .unwrap();
    let before = {
        let p = paragraph(&doc.nodes);
        doc.state
            .boxes
            .borrow()
            .normalized(&p.children, &p.style.used())
            .unwrap()
    };
    doc.update("<p class=b>hello world</p>", "").unwrap();
    let p = paragraph(&doc.nodes);
    let after = doc
        .state
        .boxes
        .borrow()
        .normalized(&p.children, &p.style.used())
        .unwrap();
    assert!(before.same(&after));
}

#[test]
fn retained_text_matches_css_whitespace_case_and_segment_break_rules() {
    for (attributes, text) in [
        ("style='white-space:pre'", "a&#13;\n  b\t c"),
        ("style='white-space:pre-line'", " a \n  b &#13; c"),
        ("style='white-space:break-spaces'", "a  \t b"),
        (
            "style='text-transform:capitalize'",
            "hel<!--boundary-->lo world",
        ),
        (
            "lang=tr style='text-transform:uppercase'",
            "i<!--boundary-->ı",
        ),
        ("style='text-transform:full-width'", "a<!--boundary--> b"),
        ("", "a&#x200b;<!--boundary-->\n b"),
        ("", "a&#x200b;<!--boundary-->&#x200e;<!--boundary-->\n b"),
    ] {
        let doc = Document::builder(&format!("<p {attributes}>{text}</p>"), "")
            .build()
            .unwrap();
        let p = paragraph(&doc.nodes);
        let style = BoxStyle::shared(p.style.used().style);
        doc.state
            .boxes
            .borrow_mut()
            .finish(&doc.nodes, &BoxStyle::default(), false);
        let normalized = doc
            .state
            .boxes
            .borrow()
            .normalized(&p.children, &style)
            .unwrap();
        let canonical = strings(inline::collect(
            &p.children,
            &style,
            &mut |_, _| None,
            &LayoutCx::default(),
        ));
        let runs = normalized
            .items()
            .iter()
            .map(|item| {
                let ItemKind::Text { start, end } = item.kind else {
                    panic!("text run")
                };
                assert!(doc.state.boxes.borrow().get(item.box_id).is_some());
                normalized.text()[start as usize..end as usize].to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(runs, canonical, "{attributes}: {text:?}");
    }
}
