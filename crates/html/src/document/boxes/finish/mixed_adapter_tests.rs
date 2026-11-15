//! The paragraph adapter takes the retained runs of a flow with inline boxes
//! and lowers exactly the pieces of node-local normalization.

use super::tests::paragraph;
use super::*;
use crate::document::Document;
use crate::layout::cx::{BuildDoc, LayoutCx, ValueContext};
use crate::text::inline::Piece;
use crate::text::inline::Runs;

/// Text of every piece, `None` for a piece without text.
fn texts(pieces: &[Piece]) -> Vec<Option<String>> {
    pieces
        .iter()
        .map(|piece| match piece {
            Piece::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Pieces of the paragraph of `html`, with the box tree finished or not.
fn pieces(html: &str, retained: bool) -> (Vec<Option<String>>, bool) {
    let doc = Document::builder(html, "").build().unwrap();
    crate::style::cascade::materialize::materialize(
        &doc.nodes,
        &Rc::new(Computed::default()),
        doc.values(),
    );
    if retained {
        BoxTree::edit(&doc.state.boxes).finish(&doc.nodes, &BoxStyle::default(), true);
    }
    let p = paragraph(&doc.nodes);
    let style = BoxStyle::shared(p.style.used().style);
    let cx = LayoutCx {
        values: ValueContext {
            doc: BuildDoc::of(doc.state.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    let none = &mut |_: &crate::dom::Element, _: &_| None;
    let out = crate::text::inline::collect(&p.children, &style, none, &cx);
    let mixed = crate::text::inline::collect_mixed(&p.children, &style, none, &cx);
    if let Some(mixed) = &mixed {
        assert_eq!(texts(mixed), texts(&out));
    }
    (texts(&out), mixed.is_some())
}

#[test]
fn retained_runs_lower_the_same_pieces_as_node_normalization() {
    for html in [
        "<p style='text-transform:capitalize'>hel<b>lo</b> <i>école</i> A<span>V</span> سلام</p>",
        "<p>  a <b>  b  </b>  <span style='white-space:pre'> c  d </span> e\n f</p>",
        "<p style='padding:0 4px'>x <span style='padding:0 3px;border:1px solid'>y</span> z</p>",
        "<p>one<span style='text-transform:uppercase'>two <i>three</i></span>four</p>",
        "<p dir=rtl>abc <span dir=ltr>def</span> ghi</p>",
    ] {
        let (retained, mixed) = pieces(html, true);
        let (fresh, _) = pieces(html, false);
        assert!(mixed, "a mixed flow is prepared: {html}");
        assert_eq!(retained, fresh, "{html}");
    }
}

#[test]
fn runs_drop_out_on_other_text_or_inputs_and_leave_nothing_half_used() {
    let html = "<p>a <b>b</b> c</p>";
    let doc = Document::builder(html, "").build().unwrap();
    crate::style::cascade::materialize::materialize(
        &doc.nodes,
        &Rc::new(Computed::default()),
        doc.values(),
    );
    BoxTree::edit(&doc.state.boxes).finish(&doc.nodes, &BoxStyle::default(), true);
    let p = paragraph(&doc.nodes);
    let style = BoxStyle::shared(p.style.used().style);
    let tree = doc.state.boxes.borrow();
    let mut upper = style.clone();
    upper.inh.text_flow.text_transform = Some(crate::style::computed::TextTransform::Upper);
    assert!(tree.mixed_text(&p.children, &upper).is_none());
    let b = match &p.children[1] {
        Node::Element(b) => b.style.used().style,
        _ => panic!("b"),
    };
    let runs = || Runs::new(tree.mixed_text(&p.children, &style).unwrap());
    let mut taken = runs();
    assert_eq!(taken.next("a ", &style), Some("a "));
    assert!(!taken.complete());
    assert_eq!(taken.next("b", &b), Some("b"));
    assert_eq!(taken.next(" c", &style), Some(" c"));
    assert!(taken.complete());
    let mut other = runs();
    assert_eq!(other.next("x ", &style), None);
    assert_eq!(other.next("b", &b), None, "a dropped run stays dropped");
    assert!(!other.complete());
    let mut restyled = runs();
    restyled.next("a ", &style);
    assert_eq!(restyled.next("b", &upper), None);
    assert!(!restyled.complete());
}
