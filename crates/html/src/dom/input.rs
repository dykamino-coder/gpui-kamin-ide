//! Ограничивает глубину HTML-дерева до рекурсии селекторов, сбора стилей и конвертации в DOM.

use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{NodeData, RcDom};

// Bound adversarial nesting before recursive consumers, as HTML parsers do.
// Our style frames exceed Blink's parser frames: 512 still overflows a native
// test thread when building a Document. Keep a tested engine-wide budget of 64.
// The document node is depth zero.
pub(crate) const MAX_DEPTH: usize = 64;

pub(crate) fn html(source: &str) -> RcDom {
    let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(source);
    bound(&dom);
    dom
}

pub(crate) fn bound(dom: &RcDom) {
    let mut pending = vec![(dom.document.clone(), 0)];
    while let Some((node, depth)) = pending.pop() {
        if depth >= MAX_DEPTH {
            node.children.borrow_mut().clear();
            if let NodeData::Element {
                template_contents, ..
            } = &node.data
            {
                template_contents.borrow_mut().take();
            }
            continue;
        }
        pending.extend(
            node.children
                .borrow()
                .iter()
                .map(|child| (child.clone(), depth + 1)),
        );
        if let NodeData::Element {
            template_contents, ..
        } = &node.data
            && let Some(contents) = template_contents.borrow().as_ref()
        {
            pending.push((contents.clone(), depth + 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deeply_nested_input_is_bounded_before_recursive_walks() {
        let source = format!("{}text{}", "<div>".repeat(10_000), "</div>".repeat(10_000));
        let dom = html(&source);
        #[expect(
            clippy::redundant_clone,
            reason = "keep the root alive: Node::drop drains descendant children"
        )]
        let mut pending = vec![(dom.document.clone(), 0)];
        let mut maximum = 0;
        while let Some((node, depth)) = pending.pop() {
            maximum = maximum.max(depth);
            pending.extend(
                node.children
                    .borrow()
                    .iter()
                    .map(|child| (child.clone(), depth + 1)),
            );
        }
        assert_eq!(maximum, MAX_DEPTH);
        let nodes = super::super::parse(&source, "");
        assert!(!nodes.is_empty());
        let document = crate::document::Document::new_with_base(&source, "", "file:///deep.html");
        assert!(document.node_count() <= MAX_DEPTH);
    }

    #[test]
    fn ordinary_html_keeps_its_text() {
        let dom = html("<p>Hello <b>world</b></p>");
        #[expect(
            clippy::redundant_clone,
            reason = "keep the root alive: Node::drop drains descendant children"
        )]
        let mut pending = vec![dom.document.clone()];
        let mut text = String::new();
        while let Some(node) = pending.pop() {
            if let NodeData::Text { contents } = &node.data {
                text.push_str(&contents.borrow());
            }
            pending.extend(node.children.borrow().iter().rev().cloned());
        }
        assert_eq!(text, "Hello world");
    }
}
