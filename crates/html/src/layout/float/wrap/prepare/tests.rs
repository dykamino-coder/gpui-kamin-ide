//! Float margins use computed font metrics before lowering into a band host.

use super::*;

#[test]
fn font_relative_float_margins_are_computed_without_changing_percentage_bases() {
    let doc = crate::document::state::DocState::new(Default::default());
    let parent = Computed {
        font_size: Some(Len::Px(20.)),
        monospace: Some(true),
        ..Default::default()
    };
    let mut style = Computed {
        float: Some(-1),
        font_size: Some(Len::Pct(2.)),
        ..Default::default()
    };
    style.margin.left = Some(Len::Ch(2.));
    style.margin.right = Some(Len::Pct(0.1));
    let resolved = crate::style::cascade::inherit::inherit_unpainted(&parent, &style, doc.values());
    let nodes = prepare_float_nodes(
        vec![Node::Element(Element::synthetic(
            "div",
            style,
            Default::default(),
            vec![],
        ))],
        &parent,
        false,
        None,
        doc.values(),
    );
    let Node::Element(float) = &nodes[0] else {
        panic!("float")
    };
    assert_eq!(float.style.margin.left, resolved.margin.left);
    assert!(matches!(float.style.margin.left, Some(Len::Px(v)) if v > 0.));
    assert_eq!(float.style.margin.right, Some(Len::Pct(0.1)));
    assert_eq!(float.style.font_size, Some(Len::Pct(2.)));
}

#[test]
fn computed_margins_admit_a_clamped_float_without_changing_its_own_context() {
    let doc = crate::document::state::DocState::new(Default::default());
    let parent = Computed {
        font_size: Some(Len::Px(20.)),
        monospace: Some(true),
        ..Default::default()
    };
    let mut style = Computed {
        float: Some(-1),
        line_clamp: Some(2),
        width: Some(Len::Ch(8.5)),
        ..Default::default()
    };
    style.margin.left = Some(Len::Ch(1.));
    let float = Element::synthetic(
        "div",
        style,
        Default::default(),
        vec![Node::Text("TEST TEST TEST".into())],
    );
    let nodes = prepare_float_nodes(
        vec![Node::Element(float)],
        &parent,
        false,
        None,
        doc.values(),
    );
    let (host, next, _, _) = host_for_run(
        20.,
        true,
        FloatCb::of(&parent),
        &nodes,
        &[],
        0,
        doc.values(),
    )
    .expect("computed margins belong to the shared host");
    assert_eq!(next, nodes.len());
    assert!(host.marks.shape_flow.is_some());
    let Node::Element(float) = &host.children[0] else {
        panic!("float")
    };
    assert_eq!(float.style.clamp_lines(), Some(2));
    assert_eq!(float.style.width, Some(Len::Ch(8.5)));
    assert!(matches!(float.style.margin.left, Some(Len::Px(v)) if v > 0.));
}

#[test]
fn float_only_inline_keeps_one_margin_fragment_before_its_floats() {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut inline = Element::synthetic(
        "span",
        Computed::default(),
        Default::default(),
        (0..2)
            .map(|_| {
                Node::Element(Element::synthetic(
                    "span",
                    Computed {
                        float: Some(-1),
                        width: Some(Len::Pct(0.5)),
                        ..Default::default()
                    },
                    Default::default(),
                    vec![],
                ))
            })
            .collect(),
    );
    inline.inline = true;
    inline.style.margin.left = Some(Len::Px(40.));
    inline.children.insert(1, Node::Text("\n ".into()));
    for (newlines, spaces) in [(true, false), (false, true)] {
        for inherited in [false, true] {
            let mut kept = inline.clone();
            let mut parent = Computed::default();
            let style = if inherited {
                &mut parent
            } else {
                &mut kept.style
            };
            style.preserve_newlines = Some(newlines);
            style.keep_spaces = Some(spaces);
            let lifted = inline_floats::lift(vec![Node::Element(kept)], &parent, doc.values());
            assert_eq!(
                lifted.len(),
                1,
                "preserved whitespace must stay in its inline"
            );
            let Node::Element(kept) = &lifted[0] else {
                panic!("preserved inline")
            };
            assert_eq!(kept.children.len(), 3);
            assert!(matches!(&kept.children[1], Node::Text(t) if t == "\n "));
        }
    }
    let nodes = prepare_float_nodes(
        vec![Node::Element(inline)],
        &Computed::default(),
        false,
        Some(784.),
        doc.values(),
    );
    assert_eq!(nodes.len(), 3);
    let Node::Element(fragment) = &nodes[0] else {
        panic!("inline fragment")
    };
    assert!(fragment.children.is_empty());
    assert_eq!(fragment.style.margin.left, Some(Len::Px(40.)));
    for node in &nodes[1..] {
        let Node::Element(float) = node else {
            panic!("float")
        };
        assert_eq!(float.style.float, Some(-1));
        assert_eq!(float.style.width, Some(Len::Pct(0.5)));
        assert_eq!(float.style.margin.left, None);
    }
    let (host, end, lead, _) = host_for_run(
        16.,
        true,
        FloatCb::of(&Computed::default()),
        &nodes,
        &nodes[..1],
        1,
        doc.values(),
    )
    .expect("shared host");
    assert_eq!(end, nodes.len());
    assert_eq!(lead, Some(0));
    assert_eq!(host.marks.shape_flow.expect("host").count, 2);
}
