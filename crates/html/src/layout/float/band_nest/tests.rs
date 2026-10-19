//! Nested float paragraph construction regressions.

use super::*;

#[test]
fn nested_float_keeps_its_paragraph_and_completed_line_probe() {
    let element = |tag, style, children| {
        Node::Element(Element::synthetic(tag, style, Default::default(), children))
    };
    let seq = band_seq(
        vec![
            element("div", Computed::default(), vec![Node::Text("block".into())]),
            Node::Text("first".into()),
            element("br", Computed::default(), vec![]),
            Node::Text("before".into()),
            element(
                "div",
                Computed {
                    float: Some(-1),
                    ..Default::default()
                },
                vec![],
            ),
            Node::Text("after".into()),
        ],
        16.,
        WritingMode::default(),
        Some(400.),
        false,
    )
    .unwrap();
    let float = seq
        .iter()
        .position(|n| {
            matches!(n, Node::Element(c)
        if c.style.float == Some(-1))
        })
        .unwrap();
    assert_eq!(float, 1);
    let probes: Vec<_> = seq
        .iter()
        .filter_map(|n| match n {
            Node::Element(c) if c.marks.lead.is_some() => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(probes.len(), 2);
    assert!(probes.iter().all(|c| {
        c.marks
            .lead
            .is_some_and(|p| p.from == float && p.to == float + 1)
    }));
    assert!(matches!(&probes[0].children[0], Node::Text(t) if t == "before"));
    assert!(probes[1].marks.lead.unwrap().base);
    assert!(seq.iter().any(|n| matches!(n, Node::Element(c) if c.marks.anon
        && c.marks.lead.is_none() && c.children.iter().any(|n| matches!(n, Node::Text(t) if t == "after")))));
}
