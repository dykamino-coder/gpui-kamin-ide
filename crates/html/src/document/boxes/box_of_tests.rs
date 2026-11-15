//! Layout finds the box of a source element by its key in O(1).

use crate::document::Document;
use crate::dom::Node;

#[test]
fn every_keyed_element_finds_its_first_box_and_an_edit_keeps_it() {
    let html = "<div><p>a <b>b</b></p><ul><li>x</li><li>y</li></ul></div>";
    let mut doc = Document::builder(html, "").build().unwrap();
    let check = |doc: &Document| {
        let tree = doc.state.boxes.borrow();
        let mut pending = vec![doc.nodes.iter()];
        let mut seen = 0;
        while let Some(siblings) = pending.last_mut() {
            match siblings.next() {
                Some(Node::Element(e)) => {
                    if e.node_id != 0 {
                        let id = tree.box_of(e.node_id).expect("keyed element has a box");
                        assert_eq!(tree.get(id).map(|r| r.source), Some(e.node_id));
                        seen += 1;
                    }
                    pending.push(e.children.iter());
                }
                Some(Node::Text(_)) => {}
                None => {
                    pending.pop();
                }
            }
        }
        assert!(seen >= 6, "{seen}");
        assert_eq!(tree.box_of(0), None);
    };
    check(&doc);
    let b = |doc: &Document| {
        let tree = doc.state.boxes.borrow();
        tree.iter()
            .find(|(_, r)| r.inline.is_none() && !r.first.is_some() && r.source != 0)
            .map(|(id, r)| (r.source, id))
    };
    let before = b(&doc);
    doc.update(
        "<div><p>a <b class=x>b</b></p><ul><li>x</li><li>y</li></ul></div>",
        "",
    )
    .unwrap();
    check(&doc);
    let (source, id) = before.unwrap();
    assert_eq!(
        doc.state.boxes.borrow().box_of(source),
        Some(id),
        "an attribute edit keeps the box"
    );
}
