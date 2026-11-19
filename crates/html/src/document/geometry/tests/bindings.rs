use super::*;

#[test]
fn two_live_box_ids_for_one_source_cannot_donate_its_scroll_offset() {
    let doc = Document::builder("<div id=a>a</div>text<!--gap--><div id=b>b</div>", "")
        .build()
        .unwrap();
    let root = doc.source_dom();
    let elements = doc.state.values.traversal.elements();
    let a = elements
        .iter()
        .copied()
        .find(|address| *address != 0)
        .unwrap();
    let mut live = elements.to_vec();
    live.push(a);
    let (query, scroll) = super::super::bindings(&root, &elements, &live);
    let id = query[&a];
    assert!(
        !scroll.values().any(|target| *target == id),
        "an ambiguous source transferred scroll to query box {id}"
    );
    assert!(scroll.values().any(|target| *target != id));
}

#[test]
fn source_bindings_match_ids_written_on_real_boxes() {
    let html = "<!-- gap --><div id=a>first</div>text<section style='display:none'><i></i></section><div id=b>second</div>";
    let doc = Document::builder(html, BROWSER_CSS).build().unwrap();
    let root = doc.source_dom();
    let built = doc.state.values.traversal.elements();
    let (sources, scroll) = super::super::bindings(&root, &built, &built);
    let mut boxes = std::collections::HashMap::new();
    let mut children: Vec<_> = doc.nodes.iter().collect();
    while let Some(node) = children.pop() {
        if let crate::dom::Node::Element(element) = node {
            if let Some(id) = element.attr("id") {
                boxes.insert(id, element.node_id);
            }
            children.extend(element.children.iter());
        }
    }
    let mut matched = Vec::new();
    let mut nodes = vec![root];
    while let Some(node) = nodes.pop() {
        nodes.extend(node.children.borrow().iter().cloned());
        let markup5ever_rcdom::NodeData::Element { attrs, .. } = &node.data else {
            continue;
        };
        let attrs = attrs.borrow();
        if let Some(attr) = attrs.iter().find(|attr| &*attr.name.local == "id") {
            let id = boxes[&*attr.value];
            assert_eq!(sources.get(&(Rc::as_ptr(&node) as usize)), Some(&id));
            assert_eq!(scroll.get(&id), Some(&id));
            matched.push(id);
        }
    }
    assert_eq!(matched.len(), 2);
    assert_ne!(matched[0], matched[1]);
}
