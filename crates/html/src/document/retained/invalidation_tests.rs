//! The block records over a rebuild for a batch of attribute mutations: only
//! the boxes of the elements it restyled are compared (`Store::rebase`).

use super::reuse_tests::{reused, update};
use super::tests::{View, frame, open};
use crate::dom::{Element, Node};
use gpui::{TestAppContext, VisualTestContext};

/// A long flow whose class on `html` recolours `body` alone.
fn page(class: &str, text: &str) -> String {
    let rows: String = (0..600)
        .map(|i| {
            let body = if i == 300 { text } else { "row" };
            format!("<div class=r>{body} {i}</div>")
        })
        .collect();
    format!(
        "<!doctype html><html class='{class}'><head><style>\
         html,body{{margin:0;padding:0;background:#fff;color:#000}} .r{{height:20px}} \
         html.paint body{{color:#123456}}</style></head><body>{rows}</body></html>"
    )
}

fn settled(cx: &mut TestAppContext) -> (gpui::Entity<View>, &mut VisualTestContext) {
    let (view, cx) = open(cx, &page("", "row"));
    frame(cx);
    update(&view, cx, &page("", "edited"));
    update(&view, cx, &page("", "row"));
    (view, cx)
}

#[gpui::test]
fn a_recoloured_body_repaints_the_offscreen_records(cx: &mut TestAppContext) {
    let (view, cx) = settled(cx);
    let (_, skipped_before, _) = reused(&view, cx);
    update(&view, cx, &page("paint", "row"));
    let (_, skipped, rebuilt) = reused(&view, cx);
    assert!(
        skipped - skipped_before > 400,
        "skipped {}",
        skipped - skipped_before
    );
    assert_eq!(rebuilt, 0);
}

/// The `body` children of `nodes`.
fn flow(nodes: &[Node]) -> &[Node] {
    let html = nodes.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "html" => Some(e),
        _ => None,
    });
    let body = html.unwrap().children.iter().find_map(|n| match n {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    });
    body.unwrap().children.as_slice()
}

/// The document tree with the text of the flow block `id` changed.
fn retexted(nodes: &[Node], id: u64) -> Vec<Node> {
    let mut nodes = nodes.to_vec();
    let html = nodes.iter_mut().find_map(|n| match n {
        Node::Element(e) if e.tag == "html" => Some(e),
        _ => None,
    });
    let html = html.unwrap();
    let mut kids = html.children.clone().into_vec();
    let body = kids.iter_mut().find_map(|n| match n {
        Node::Element(e) if e.tag == "body" => Some(e),
        _ => None,
    });
    let body = body.unwrap();
    let mut rows = body.children.clone().into_vec();
    let block = rows.iter_mut().find_map(|n| match n {
        Node::Element(e) if e.node_id == id => Some(e),
        _ => None,
    });
    let text = crate::dom::Text::from_source("edited".into(), None);
    block.unwrap().children = vec![Node::Text(text)].into();
    body.children = rows.into();
    html.children = kids.into();
    nodes
}

type Case = (
    std::rc::Rc<crate::layout::reuse::Store>,
    Vec<Node>,
    Vec<Node>,
    Element,
);

/// A settled document's records, its tree, the tree with the text of the last
/// recorded block changed (far from the screen), and that block.
fn case(cx: &mut TestAppContext) -> Case {
    let (view, cx) = settled(cx);
    let (store, nodes) = cx.update(|_, cx| {
        let doc = &view.read(cx).doc;
        (doc.reuse.clone(), doc.nodes.to_vec())
    });
    let recorded = store.recorded().into_iter().filter(|e| e.tag == "div");
    let block = recorded
        .max_by_key(|e| e.node_id)
        .expect("settled blocks were recorded");
    let changed = retexted(&nodes, block.node_id);
    assert_eq!(flow(&changed).len(), flow(&nodes).len());
    (store, nodes, changed, block)
}

#[gpui::test]
fn only_the_blocks_of_restyled_elements_are_compared(cx: &mut TestAppContext) {
    let (store, nodes, changed, block) = case(cx);
    // No element restyled: the changed block is not compared and keeps its
    // record; this tree is not one a rebuild of the batch could make.
    store.rebase(&nodes, &changed, Some(&[]));
    assert!(
        store.same_tree(&block).is_some(),
        "an unrestyled block was compared"
    );
}

#[gpui::test]
fn a_restyled_element_compares_its_block(cx: &mut TestAppContext) {
    let (store, nodes, changed, block) = case(cx);
    store.rebase(&nodes, &changed, Some(&[block.node_id]));
    assert!(
        store.same_tree(&block).is_none(),
        "a restyled changed block kept its record"
    );
}

#[gpui::test]
fn unknown_restyles_compare_every_block(cx: &mut TestAppContext) {
    let (store, nodes, changed, block) = case(cx);
    store.rebase(&nodes, &changed, None);
    assert!(
        store.same_tree(&block).is_none(),
        "a changed block kept its record"
    );
}

#[gpui::test]
fn a_restyled_block_with_new_layout_values_compares_every_block(cx: &mut TestAppContext) {
    let (store, nodes, changed, block) = case(cx);
    // Another far recorded block is restyled and changed too: what follows
    // it in document order (counters, list numbers) may have changed.
    let recorded = store.recorded().into_iter().filter(|e| e.tag == "div");
    let other = recorded
        .filter(|e| e.node_id < block.node_id)
        .max_by_key(|e| e.node_id)
        .expect("two far blocks were recorded");
    let changed = retexted(&changed, other.node_id);
    store.rebase(&nodes, &changed, Some(&[other.node_id]));
    assert!(store.same_tree(&other).is_none());
    assert!(
        store.same_tree(&block).is_none(),
        "a later block was not compared"
    );
}
