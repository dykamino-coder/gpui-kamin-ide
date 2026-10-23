//! Тесты: построение band-хоста сохраняет порядок float, ширину и продолжение абзаца.

use super::*;
use crate::dom::parse;
use crate::layout::float::band_flow_host::band_flow_rest_lift;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

pub(super) fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
    nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.attr("id") == Some(id) => Some(e),
        Node::Element(e) => find(&e.children, id),
        Node::Text(_) => None,
    })
}

pub(super) fn default_host(
    nodes: &[Node],
    vcx: crate::document::state::ValueCx<'_>,
) -> Option<(Element, usize, Vec<Node>)> {
    band_host_m(nodes, 0, 16.0, &[], FloatCb::of(&Computed::default()), vcx)
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
    assert_eq!(host.style.cascaded().width, None);
    let shape = host.marks.shape_flow.expect("band marks");
    assert_eq!(shape.cbw, Some(200.0));
    assert_eq!(shape.count, 2);
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

#[test]
fn inline_ruby_remains_in_the_measured_paragraph_across_breaks() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    for position in ["over", "under"] {
        let markup = format!(
            r#"<div id="source"><div style="float:left;width:50px;height:50px"></div><ruby id="ruby" style="ruby-position:{position}">base<rt>annotation</rt></ruby><br>next</div>"#
        );
        let tree = parse(&markup, "");
        let nodes = &find(&tree, "source").expect("source").children;
        let (host, end, lifted) =
            default_host(nodes, vcx).expect("inline ruby is measured by its paragraph");
        assert_eq!(end, nodes.len());
        assert!(lifted.is_empty());
        assert!(host.marks.shape_flow.is_some());
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
fn nested_percentage_width_keeps_one_shared_float_context() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    let tree = parse(
        r#"<div id="source"><div style="float:left;width:20px;height:10px"></div><div id="nested" style="width:200%"><div style="float:left;width:50%;height:40px"></div><div style="clear:both;width:50%"><div style="display:flow-root;height:100px"></div></div></div></div>"#,
        "",
    );
    let nodes = &find(&tree, "source").expect("source").children;
    let (host, end, _) =
        default_host(nodes, vcx).expect("percentage width does not establish an independent BFC");
    assert_eq!(end, nodes.len());
    assert!(host.marks.shape_flow.is_some());
    assert_eq!(
        find(&host.children, "nested")
            .expect("nested")
            .style
            .cascaded()
            .width,
        Some(Len::Pct(2.))
    );
}

#[test]
fn nested_pinned_absolute_keeps_the_external_containing_block() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    let tree = parse(
        r#"<div id="source"><div style="float:left;width:50px;height:50px"></div><div><div style="float:left;width:50px;height:50px"></div><div id="pin" style="position:absolute;right:0;top:40px;width:50px;height:10px"></div></div></div>"#,
        "",
    );
    let nodes = &find(&tree, "source").expect("source").children;
    let (host, end, lifted) =
        default_host(nodes, vcx).expect("pinned descendant does not depend on a hypothetical line");
    assert_eq!(end, nodes.len());
    assert!(find(&host.children, "pin").is_none());
    assert_eq!(lifted.len(), 1);
    assert!(find(&lifted, "pin").is_some());
    let pin = Node::Element(find(nodes, "pin").unwrap().clone());
    assert!(band_flow_rest_lift(vec![pin], 16., None, WritingMode::default(), None, vcx).is_none());
}

#[test]
fn block_static_positions_stay_in_the_shared_paragraph() {
    let doc = crate::document::state::DocState::new(Default::default());
    let vcx = doc.values();
    for inset in ["", "top:0;", "left:0;"] {
        let tree = parse(
            &format!(
                r#"<div id="source"><div style="float:left;width:50px;height:50px"></div><div id="oof" style="position:absolute;{inset}width:20px;height:10px"></div></div>"#
            ),
            "",
        );
        let nodes = &find(&tree, "source").unwrap().children;
        let (host, end, lifted) = default_host(nodes, vcx)
            .expect("shared paragraph owns the hypothetical block position");
        assert_eq!(end, nodes.len());
        assert!(lifted.is_empty());
        assert!(find(&host.children, "oof").is_some());
    }
}
