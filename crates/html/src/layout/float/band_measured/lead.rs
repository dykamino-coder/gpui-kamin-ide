//! Жёсткие разрывы отделяют завершённые строки от строки, предшествующей float.

use crate::dom::Node;
use crate::layout::box_kind::{atomic_inline, inline_level, out_of_flow, replaced_tag};

/// The same paragraph probes serve top-level and nested floats. Their ranges
/// refer to node positions before the non-painting probes are appended.
pub(crate) fn probes(nodes: Vec<Node>, from: usize, to: usize, preserve: bool) -> Vec<Node> {
    use crate::dom::{Element, LeadProbe, Marks};
    let (base, line) = split(nodes, preserve);
    let probe = |children, base| {
        Node::Element(Element::synthetic(
            "div",
            Default::default(),
            Marks {
                anon: true,
                lead: Some(LeadProbe { base, from, to }),
                ..Default::default()
            },
            children,
        ))
    };
    let mut out = vec![probe(line, false)];
    if let Some(base) = base {
        out.push(probe(base, true));
    }
    out
}

pub(super) fn split(mut nodes: Vec<Node>, preserve: bool) -> (Option<Vec<Node>>, Vec<Node>) {
    for i in (0..nodes.len()).rev() {
        match &nodes[i] {
            Node::Element(c) if c.tag == "br" => {
                let line = nodes.split_off(i + 1);
                return (Some(nodes), line);
            }
            Node::Element(c)
                if inline_level(c)
                    && !atomic_inline(&c.style.computed())
                    && !out_of_flow(&c.style.computed())
                    && !replaced_tag(c) =>
            {
                // CSS 2.1 section 9.5.1 rule 6: a break inside an inline
                // ancestor completes the line before the float, too. Keep
                // the ancestor's metrics on both sides of the probe split.
                let (base, line) = split(c.children.clone(), preserve);
                if let Some(base) = base {
                    let mut before = c.clone();
                    before.children = base;
                    let mut after = c.clone();
                    after.children = line;
                    let mut tail = nodes.split_off(i + 1);
                    nodes.truncate(i);
                    nodes.push(Node::Element(before));
                    if !after.children.is_empty() {
                        tail.insert(0, Node::Element(after));
                    }
                    return (Some(nodes), tail);
                }
            }
            Node::Text(text) if preserve => {
                if let Some(at) = text.rfind('\n') {
                    let after = text[at + 1..].to_owned();
                    let before = text[..at + 1].to_owned();
                    let mut line = nodes.split_off(i + 1);
                    nodes[i] = Node::Text(before);
                    line.insert(0, Node::Text(after));
                    return (Some(nodes), line);
                }
            }
            _ => {}
        }
    }
    (None, nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserved_break_keeps_completed_lines_in_the_base() {
        let (base, line) = split(vec![Node::Text("one\ntwo\n tail".into())], true);
        assert!(matches!(&base.unwrap()[0], Node::Text(t) if t == "one\ntwo\n"));
        assert!(matches!(&line[0], Node::Text(t) if t == " tail"));
    }

    #[test]
    fn collapsed_segment_break_stays_on_the_float_line() {
        let (base, line) = split(vec![Node::Text("one\ntwo".into())], false);
        assert!(base.is_none());
        assert_eq!(line.len(), 1);
    }

    #[test]
    fn nested_break_retains_inline_metrics_and_respects_atomic_boundaries() {
        use crate::dom::Element;
        use crate::style::computed::{Computed, Display};
        use crate::style::values::value::Len;
        let br = Element::synthetic("br", Computed::default(), Default::default(), vec![]);
        let mut span = Element::synthetic(
            "span",
            Computed {
                line_height: Some(Len::Px(200.)),
                ..Default::default()
            },
            Default::default(),
            vec![Node::Element(br), Node::Text("tail".into())],
        );
        span.inline = true;
        let (base, line) = split(vec![Node::Element(span.clone())], false);
        assert!(matches!(&base.unwrap()[0], Node::Element(e)
            if e.style.cascaded().line_height == Some(Len::Px(200.))
            && matches!(&e.children[0], Node::Element(br) if br.tag == "br")));
        assert!(matches!(&line[0], Node::Element(e)
            if e.style.cascaded().line_height == Some(Len::Px(200.))
            && matches!(&e.children[0], Node::Text(t) if t == "tail")));
        span.style.used_mut().display = Some(Display::InlineBlock);
        let (base, _) = split(vec![Node::Element(span)], false);
        assert!(base.is_none());
    }
}
