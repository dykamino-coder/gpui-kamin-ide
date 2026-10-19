//! Band host construction keeps float order, width and paragraph continuation.

use super::*;
use crate::dom::{BandPath, parse};
use crate::layout::float::band_flow_host::band_flow_rest_lift;
use crate::style::values::value::Len;

fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.attr("id") == Some(id) => Some(e),
        Node::Element(e) => find(&e.children, id),
        Node::Text(_) => None,
    })
}

#[test]
fn measured_host_keeps_float_order_and_containing_width() {
    let tree = parse(
        r#"<div id="source"><i id="left" style="float:left;width:20px;height:10px"></i><i id="right" style="float:right;width:30px;height:10px"></i><div id="tail" style="overflow:hidden;width:10px;height:8px">tail</div></div>"#,
        "",
    );
    let nodes = &find(&tree, "source").expect("source").children;
    let parent = crate::style::computed::Computed {
        width: Some(Len::Px(200.0)),
        ..Default::default()
    };
    let (host, end, lifted) =
        band_host_m(nodes, 0, 16.0, &[], FloatCb::of(&parent)).expect("measured host");
    assert_eq!(end, nodes.len());
    assert!(lifted.is_empty());
    assert_eq!(host.style.width, None);
    let shape = host.marks.shape_flow.expect("band marks");
    assert_eq!(shape.cbw, Some(200.0));
    assert_eq!(shape.bands, Some(BandPath::Measured));
    assert_eq!(shape.count, 2);
    assert!(!shape.right);
    let ids: Vec<_> = host
        .children
        .iter()
        .map(|n| match n {
            Node::Element(e) => e.attr("id"),
            Node::Text(_) => None,
        })
        .collect();
    assert_eq!(ids, [Some("left"), Some("right"), Some("tail")]);
    assert_eq!(host.node_id, 0);
    assert!(host.attrs.is_empty());
    assert!(!host.inline);
}

#[test]
fn lifted_inline_runs_keep_paragraph_continuation() {
    let tree = parse(r#"<div id="source">first<br>next</div>"#, "");
    let nodes = find(&tree, "source").expect("source").children.clone();
    let runs =
        band_flow_rest_lift(nodes, 16.0, None, WritingMode::default(), None).expect("inline runs");
    assert_eq!(runs.len(), 2);
    let marks: Vec<_> = runs
        .iter()
        .map(|n| match n {
            Node::Element(e) => {
                assert!(e.marks.anon);
                assert!(!e.marks.atoms);
                assert!(!e.inline);
                e.marks.cont
            }
            Node::Text(_) => panic!("run must be an anonymous box"),
        })
        .collect();
    assert_eq!(marks, [false, true]);
}

#[test]
fn orthogonal_block_tail_keeps_its_independent_contents_in_the_measured_host() {
    for mode in ["vertical-lr", "vertical-rl"] {
        let markup = format!(
            r#"<div id="source"><div style="float:right;width:40px;height:120px"></div><div id="tail" style="writing-mode:{mode};width:40px;height:120px"><ruby>base<rt>note</rt></ruby></div></div>"#
        );
        let tree = parse(&markup, "");
        let nodes = &find(&tree, "source").expect("source").children;
        let (host, end, lifted) = band_host_m(
            nodes,
            0,
            16.,
            &[],
            FloatCb::of(&crate::style::computed::Computed::default()),
        )
        .expect("orthogonal block owns its line layout");
        assert_eq!(end, nodes.len());
        assert!(lifted.is_empty());
        assert_eq!(
            host.marks.shape_flow.expect("host").bands,
            Some(BandPath::Measured)
        );
        let tail = find(&host.children, "tail").expect("tail");
        assert!(tail.children.iter().any(has_ruby));
        assert_eq!(tail.style.height, Some(Len::Px(120.)));
    }
}

#[test]
fn inline_ruby_remains_in_the_measured_paragraph_across_breaks() {
    for position in ["over", "under"] {
        let markup = format!(
            r#"<div id="source"><div style="float:left;width:50px;height:50px"></div><ruby id="ruby" style="ruby-position:{position}">base<rt>annotation</rt></ruby><br>next</div>"#
        );
        let tree = parse(&markup, "");
        let nodes = &find(&tree, "source").expect("source").children;
        let (host, end, lifted) = band_host_m(
            nodes,
            0,
            16.,
            &[],
            FloatCb::of(&crate::style::computed::Computed::default()),
        )
        .expect("inline ruby is measured by its paragraph");
        assert_eq!(end, nodes.len());
        assert!(lifted.is_empty());
        assert_eq!(
            host.marks.shape_flow.expect("host").bands,
            Some(BandPath::Measured)
        );
        let runs: Vec<_> = host
            .children
            .iter()
            .filter_map(|node| match node {
                Node::Element(e) if e.marks.anon => Some(e),
                _ => None,
            })
            .collect();
        assert_eq!(runs.len(), 2);
        assert!(!runs[0].marks.atoms && !runs[0].marks.cont);
        assert!(runs[1].marks.cont);
        let ruby = find(&runs[0].children, "ruby").expect("ruby stays in its line");
        assert!(
            ruby.children
                .iter()
                .any(|node| matches!(node, Node::Element(e) if e.tag == "rt"))
        );
    }
}

#[test]
fn orthogonal_float_roots_keep_native_layout_ownership() {
    for mode in ["vertical-lr", "vertical-rl", "sideways-lr"] {
        for (layout, content, admitted) in [
            ("", "<span>first second</span>", true),
            ("", "<div>first</div><div>second</div>", true),
            ("", r#"<span style="float:left">nested</span>"#, true),
            ("width:100px", "<div>first</div><div>second</div>", true),
            ("height:100px", "<div>first</div><div>second</div>", true),
            ("display:grid", "<div>first</div><div>second</div>", true),
            ("display:flex", "<div>first</div><div>second</div>", true),
        ] {
            let markup = format!(
                r#"<div id="source"><div style="float:left;writing-mode:{mode};{layout}">{content}</div><div style="float:right;writing-mode:{mode};{layout}">{content}</div></div>"#
            );
            let tree = parse(&markup, "");
            let nodes = &find(&tree, "source").expect("source").children;
            let measured = band_host_m(
                nodes,
                0,
                16.,
                &[],
                FloatCb::of(&crate::style::computed::Computed::default()),
            );
            if !admitted {
                assert!(measured.is_none(), "block children require root sizing");
            } else {
                let (host, end, lifted) = measured.expect("root owns its intrinsic layout");
                assert_eq!(end, nodes.len());
                assert!(lifted.is_empty());
                assert_eq!(
                    host.marks.shape_flow.expect("host").bands,
                    Some(BandPath::Measured)
                );
            }
        }
    }
}

#[test]
fn nested_percentage_width_keeps_one_shared_float_context() {
    let tree = parse(
        r#"<div id="source"><div style="float:left;width:20px;height:10px"></div><div id="nested" style="width:200%"><div style="float:left;width:50%;height:40px"></div><div style="clear:both;width:50%"><div style="display:flow-root;height:100px"></div></div></div></div>"#,
        "",
    );
    let nodes = &find(&tree, "source").expect("source").children;
    let (host, end, _) = band_host_m(
        nodes,
        0,
        16.,
        &[],
        FloatCb::of(&crate::style::computed::Computed::default()),
    )
    .expect("percentage width does not establish an independent BFC");
    assert_eq!(end, nodes.len());
    assert_eq!(
        host.marks.shape_flow.expect("host").bands,
        Some(BandPath::Measured)
    );
    assert_eq!(
        find(&host.children, "nested").expect("nested").style.width,
        Some(Len::Pct(2.))
    );
}

#[test]
fn nested_pinned_absolute_keeps_the_external_containing_block() {
    let tree = parse(
        r#"<div id="source"><div style="float:left;width:50px;height:50px"></div><div><div style="float:left;width:50px;height:50px"></div><div id="pin" style="position:absolute;right:0;top:40px;width:50px;height:10px"></div></div></div>"#,
        "",
    );
    let nodes = &find(&tree, "source").expect("source").children;
    let (host, end, lifted) = band_host_m(
        nodes,
        0,
        16.,
        &[],
        FloatCb::of(&crate::style::computed::Computed::default()),
    )
    .expect("pinned descendant does not depend on a hypothetical line");
    assert_eq!(end, nodes.len());
    assert!(find(&host.children, "pin").is_none());
    assert_eq!(lifted.len(), 1);
    assert!(find(&lifted, "pin").is_some());
}
