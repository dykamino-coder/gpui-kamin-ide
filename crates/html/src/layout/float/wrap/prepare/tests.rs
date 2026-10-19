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
    let (host, next, _, _) = host_for_run(20., true, FloatCb::of(&parent), &nodes, &[], 0)
        .expect("computed margins belong to the shared host");
    assert_eq!(next, nodes.len());
    assert_eq!(
        host.marks.shape_flow.expect("host").bands,
        Some(crate::dom::BandPath::Measured)
    );
    let Node::Element(float) = &host.children[0] else {
        panic!("float")
    };
    assert_eq!(float.style.clamp_lines(), Some(2));
    assert_eq!(float.style.width, Some(Len::Ch(8.5)));
    assert!(matches!(float.style.margin.left, Some(Len::Px(v)) if v > 0.));
}
