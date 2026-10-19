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
    let metric_doc = crate::document::state::DocState::new(Default::default());
    let vcx = metric_doc.values();
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
        band_host_m(nodes, 0, 16.0, &[], FloatCb::of(&parent), vcx).expect("measured host");
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
    let metric_doc = crate::document::state::DocState::new(Default::default());
    let vcx = metric_doc.values();
    let tree = parse(r#"<div id="source">first<br>next</div>"#, "");
    let nodes = find(&tree, "source").expect("source").children.clone();
    let runs = band_flow_rest_lift(nodes, 16.0, None, WritingMode::default(), None, vcx)
        .expect("inline runs");
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
