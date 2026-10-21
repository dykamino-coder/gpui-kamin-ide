//! Тесты-регрессии построения абзацев вложенных float.

use super::*;

#[test]
fn nested_float_keeps_its_paragraph_and_completed_line_probe() {
    let element = |tag, style, children| {
        Node::Element(Element::synthetic(tag, style, Default::default(), children))
    };
    let doc = crate::document::state::DocState::new(Default::default());
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
        doc.values(),
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

#[test]
fn percentage_nest_uses_its_definite_containing_block_height() {
    let c = Element::synthetic(
        "div",
        Computed {
            height: Some(Len::Pct(0.6)),
            ..Default::default()
        },
        Default::default(),
        vec![],
    );
    for (basis, expected) in [(Some(200.), Some(120.)), (None, None)] {
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800., 600.),
            normal_line_height: 1.2,
            cx: crate::layout::cx::LayoutCx {
                avail_h: basis,
                ..Default::default()
            },
            frag: Default::default(),
        };
        let nest = band_nest(&c, &Computed::default(), &opts, 16., WritingMode::default())
            .expect("percentage block remains in the shared BFC");
        match (nest.height, expected) {
            (Some(actual), Some(expected)) => assert!((actual - expected).abs() < 0.001),
            (actual, expected) => assert_eq!(actual, expected),
        }
        assert_eq!(opts.cx.avail_h, basis);
    }
}

#[test]
fn paint_group_nests_unless_an_absolute_descendant_needs_it() {
    let float = Element::synthetic(
        "div",
        Computed {
            float: Some(-1),
            ..Default::default()
        },
        Default::default(),
        vec![],
    );
    let group = |extra: Vec<Node>| {
        let mut children = vec![Node::Element(float.clone()), Node::Text("text".into())];
        children.extend(extra);
        Element::synthetic(
            "div",
            Computed {
                opacity: Some(0.5),
                ..Default::default()
            },
            Default::default(),
            children,
        )
    };
    let wm = WritingMode::default();
    assert!(paint_group(&group(vec![]).style));
    assert!(band_nest_block(&group(vec![]), 16., wm));
    let absolute = Element::synthetic(
        "div",
        Computed {
            position: Some(crate::style::computed::Position::Absolute),
            ..Default::default()
        },
        Default::default(),
        vec![],
    );
    assert!(!band_nest_block(
        &group(vec![Node::Element(absolute)]),
        16.,
        wm
    ));
    assert!(!paint_group(&Computed::default()));
}
