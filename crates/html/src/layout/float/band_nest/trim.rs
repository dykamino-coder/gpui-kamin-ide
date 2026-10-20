//! Trim ownership of anonymous paragraphs in the shared BFC sequence.

use crate::dom::{Element, Node};
use crate::render::{holds_line_box, is_blank, out_of_flow};
use crate::style::computed::Computed;

/// CSS Inline 3: the last formatted line owns end trimming; clearance after
/// its forced break is not trimmed. Transfer ownership to the anonymous
/// paragraph before the generated clearing box, so the canonical block trim
/// applies before BandFlow computes clearance. An authored empty block still
/// stops the search (the first/last formatted line rules).
pub(super) fn route_end(seq: &mut [Node], inherited: &Computed) {
    if !inherited.text_box_trim_end {
        return;
    }
    for n in seq.iter_mut().rev() {
        if is_blank(n) {
            continue;
        }
        let Node::Element(c) = n else { return };
        if out_of_flow(&c.style) || c.marks.lead.is_some() || clearing(c) {
            continue;
        }
        if c.marks.anon && holds_line_box(&c.children) {
            c.style.text_box_trim_end = true;
        }
        return;
    }
}

fn clearing(c: &Element) -> bool {
    c.marks.anon && c.style.clear.is_some() && c.children.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_trim_belongs_to_the_line_before_clearance() {
        let anon = |style, children| {
            Node::Element(Element::synthetic(
                "div",
                style,
                crate::dom::Marks {
                    anon: true,
                    ..Default::default()
                },
                children,
            ))
        };
        let line = anon(Computed::default(), vec![Node::Text("last line".into())]);
        let clear = anon(
            Computed {
                clear: Some(0),
                ..Default::default()
            },
            vec![],
        );
        let inherited = Computed {
            text_box_trim_end: true,
            ..Default::default()
        };
        let mut seq = vec![line.clone(), clear];
        route_end(&mut seq, &inherited);
        assert!(matches!(&seq[0], Node::Element(c) if c.style.text_box_trim_end));
        assert!(matches!(&seq[1], Node::Element(c) if !c.style.text_box_trim_end));
        // Authored empty boxes, unlike generated clearance, stop ownership.
        let mut blocked = vec![
            line,
            Node::Element(Element::synthetic(
                "div",
                Computed::default(),
                Default::default(),
                vec![],
            )),
        ];
        route_end(&mut blocked, &inherited);
        assert!(matches!(&blocked[0], Node::Element(c) if !c.style.text_box_trim_end));
    }
}
