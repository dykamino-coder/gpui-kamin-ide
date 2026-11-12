//! Tests of the paragraph piece collection (collect_with_empty_metrics).

use crate::document::parsed::parse;
use crate::dom::Node;
use crate::layout::cx::{BuildDoc, LayoutCx};
use crate::style::computed::BoxStyle;
use crate::text::inline::{self, Piece};
use gpui::IntoElement;

#[test]
fn positioned_inline_identity_reaches_anchor_and_absolute_atom() {
    let parsed = parse(
        "<span style='position:relative'><span style='anchor-name:--a'>a</span>\
         <span style='position:absolute;left:anchor(--a left)'></span></span>",
        "",
    );
    fn positioned(nodes: &[Node]) -> Option<&crate::dom::Element> {
        nodes.iter().find_map(|node| match node {
            Node::Element(e)
                if e.style.used().reset_positioned.position
                    == Some(crate::style::computed::Position::Relative) =>
            {
                Some(e)
            }
            Node::Element(e) => positioned(&e.children),
            Node::Text(_) => None,
        })
    }
    let parent = positioned(&parsed.nodes).expect("positioned inline parent");
    assert_ne!(parent.node_id, 0);
    let cx = LayoutCx {
        values: crate::layout::cx::ValueContext {
            doc: BuildDoc::of(parsed.doc.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut seen_atom = false;
    let pieces = inline::collect(
        &parsed.nodes,
        &BoxStyle::default(),
        &mut |e, atom| {
            if crate::layout::box_kind::out_of_flow(&e.style.used()) {
                assert!(atom.in_inline_cb);
                assert_eq!(atom.parent.as_ref().unwrap().self_node, parent.node_id);
                seen_atom = true;
                Some(Piece::Overlay(
                    gpui::Empty.into_any_element(),
                    Default::default(),
                ))
            } else {
                None
            }
        },
        &cx,
    );
    assert!(seen_atom);
    let anchor = pieces
        .iter()
        .find_map(|piece| match piece {
            Piece::Overlay(_, how) => how
                .inline_box_info
                .as_ref()
                .filter(|info| !info.names.is_empty()),
            _ => None,
        })
        .expect("named inline anchor");
    assert_eq!(anchor.own_cb, parent.node_id);
}

/// A paragraph inside an atom of another collects on a second stack;
/// both go back to the pool and later paragraphs reuse them.
#[test]
fn collections_reuse_the_stacks_of_finished_ones() {
    let parsed = parse("<p>a<span>b</span>c</p>", "");
    let cx = LayoutCx::default();
    let run = || {
        inline::collect(
            &parsed.nodes,
            &BoxStyle::default(),
            &mut |e, _| {
                (e.tag == "span").then(|| {
                    let inner =
                        inline::collect(&e.children, &BoxStyle::default(), &mut |_, _| None, &cx);
                    assert!(!inner.is_empty());
                    Piece::Atom(gpui::Empty.into_any_element())
                })
            },
            &cx,
        )
    };
    super::STACKS.with(|stacks| stacks.borrow_mut().clear());
    assert!(!run().is_empty());
    assert_eq!(super::STACKS.with(|stacks| stacks.borrow().len()), 2);
    assert!(!run().is_empty());
    assert_eq!(super::STACKS.with(|stacks| stacks.borrow().len()), 2);
}
/// `multicol-fill-balance-031`: the white space closing a ruby container
/// swallows the space after it (css-text-3 §4.1.1, css-ruby-1 §2.4).
#[test]
fn ruby_trailing_space_collapses_the_following_space() {
    let parsed = parse("<div><ruby>\n <rt>x</rt>\n </ruby>\n FAIL</div>", "");
    fn div(nodes: &[Node]) -> Option<&crate::dom::Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => div(&e.children),
            Node::Text(_) => None,
        })
    }
    let pieces = inline::collect(
        &div(&parsed.nodes).expect("div").children,
        &BoxStyle::default(),
        &mut |e, _| (e.tag == "ruby").then(|| Piece::Atom(gpui::Empty.into_any_element())),
        &LayoutCx::default(),
    );
    let text: String = pieces
        .iter()
        .filter_map(|p| match p {
            Piece::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text.trim_end(), "FAIL");
}
