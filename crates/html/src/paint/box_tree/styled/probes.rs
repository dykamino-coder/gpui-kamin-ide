//! Boolean paint eligibility probes own their temporary elements until the answer is known.
//!
//! These queries also run outside App::draw. Falling back to the thread arena
//! would retain every discarded decoration and its captured document owners.
//! No AnyElement escapes either scope: only an enum or a boolean is returned.

use super::{Own, group_painters, owner};
use crate::{
    dom::{Element, whitespace::is_blank},
    paint::decorations::decorations,
    style::computed::BoxStyle,
};

/// The painter requested by new used values, before program changes are
/// applied; `boxed`: `e` is built as a box of its own (`owner::painter`).
pub(crate) fn requested_painter(
    doc: &std::rc::Rc<crate::document::state::DocState>,
    e: &Element,
    boxed: bool,
) -> Own {
    let style = BoxStyle::shared(e.style.used());
    owner::painter(e, &style, boxed, || {
        gpui::with_temporary_element_arena(|| {
            let cx = crate::layout::cx::LayoutCx::default()
                .with_doc(crate::layout::cx::BuildDoc::of(doc.clone()));
            let empty = !e.children.iter().any(|n| !is_blank(n));
            !decorations(&style, empty, cx.frame(), Some(&cx)).is_empty()
        })
    })
}

/// `group_painters` exist for the new used values of `e`, before program
/// changes are applied (`requested_painter`).
pub(crate) fn requested_group_paint(
    doc: &std::rc::Rc<crate::document::state::DocState>,
    e: &Element,
) -> bool {
    gpui::with_temporary_element_arena(|| {
        let cx = crate::layout::cx::LayoutCx::default()
            .with_doc(crate::layout::cx::BuildDoc::of(doc.clone()));
        group_painters(e, &cx).is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, dom::Node};
    use std::rc::Rc;

    fn find_div(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            nodes.iter().find_map(|node| match node {
                Node::Element(e) if e.tag == "div" => Some(e),
                Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        find(nodes).expect("the fixture contains a div")
    }

    fn document() -> Document {
        Document::builder(
            "<div></div>",
            "div { display:block; border:8px solid; border-image:linear-gradient(red,blue) 1; }",
        )
        .build()
        .unwrap()
    }

    #[test]
    fn requested_painter_releases_decorations_and_their_document_capture() {
        let doc = document();
        let state = doc.state_rc();
        let e = find_div(doc.nodes());
        let owners = Rc::strong_count(&state.values);
        for _ in 0..8 {
            assert_eq!(
                requested_painter(&state, e, true),
                Own::Not(super::super::Refusal::Decoration)
            );
            assert_eq!(Rc::strong_count(&state.values), owners);
        }
    }

    #[test]
    fn requested_group_paint_releases_decorations_and_their_document_capture() {
        let doc = document();
        let state = doc.state_rc();
        let e = find_div(doc.nodes());
        let owners = Rc::strong_count(&state.values);
        let answer = requested_group_paint(&state, e);
        for _ in 0..8 {
            assert_eq!(requested_group_paint(&state, e), answer);
            assert_eq!(Rc::strong_count(&state.values), owners);
        }
    }
}
