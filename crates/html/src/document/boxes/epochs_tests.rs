//! Layout epochs: what changes the own inputs of a box, how far `child`
//! and `intrinsic` go, and that a paint-only edit leaves the epochs alone.

use super::{BoxTree, Epochs};
use crate::document::Document;
use crate::dom::{BoxId, Node};

fn node_of(nodes: &[Node], id: &str) -> Option<u64> {
    let mut pending = vec![nodes.iter()];
    while let Some(siblings) = pending.last_mut() {
        match siblings.next() {
            Some(Node::Element(e)) => {
                if e.attrs.iter().any(|(n, v)| n == "id" && v == id) {
                    return Some(e.node_id);
                }
                pending.push(e.children.iter());
            }
            Some(Node::Text(_)) => {}
            None => {
                pending.pop();
            }
        }
    }
    None
}

fn box_of(doc: &Document, id: &str) -> BoxId {
    let key = node_of(&doc.nodes, id).expect("element");
    doc.state.boxes.borrow().box_of(key).expect("box")
}

fn epochs(doc: &Document, id: &str) -> Epochs {
    doc.state
        .boxes
        .borrow()
        .epochs(box_of(doc, id))
        .expect("live")
}

fn clock(doc: &Document) -> u32 {
    doc.state.boxes.borrow().clock()
}

fn page(inner: &str, mid: &str) -> String {
    format!(
        "<div id=root><div id=float style='float:left'>\
         <div id=wrap {mid}><p id=p>{inner}</p></div></div>\
         <div id=side><p id=other>other</p></div></div>"
    )
}

#[test]
fn a_paint_only_edit_leaves_every_layout_epoch() {
    let ids = ["root", "float", "wrap", "p", "side", "other"];
    let layout = |e: Epochs| (e.layout, e.child, e.intrinsic);
    let mut doc = Document::builder(&page("text", ""), "").build().unwrap();
    let before: Vec<_> = ids.map(|id| epochs(&doc, id)).into();
    doc.update(&page("text", "style='color:red'"), "").unwrap();
    let after: Vec<_> = ids.map(|id| epochs(&doc, id)).into();
    let now = clock(&doc);
    for (i, id) in ids.into_iter().enumerate() {
        assert_eq!(layout(before[i]), layout(after[i]), "{id}");
        // The edited box and the builds of its ancestors read the colour;
        // `side` does not. `p` inherits it, which its content key compares
        // (`layout::reuse::content`), not its epoch: kept as it is.
        if id != "p" {
            let painted = !matches!(id, "side" | "other");
            assert_eq!(after[i].paint == now, painted, "{id}");
        }
    }
}

#[test]
fn a_size_edit_marks_the_box_and_its_ancestors_only() {
    let mut doc = Document::builder(&page("text", ""), "").build().unwrap();
    let side = epochs(&doc, "side");
    doc.update(&page("text", "style='width:50px'"), "").unwrap();
    let now = clock(&doc);
    assert_eq!(epochs(&doc, "wrap").layout, now);
    for id in ["float", "root"] {
        assert_eq!(epochs(&doc, id).child, now, "{id}");
        assert_ne!(epochs(&doc, id).layout, now, "{id}");
    }
    // The width changed: the contribution changes even for an externally sized box.
    assert_eq!(epochs(&doc, "float").intrinsic, now);
    assert_eq!(epochs(&doc, "side"), side);
    assert_ne!(epochs(&doc, "p").layout, now);
}

#[test]
fn a_text_edit_stops_intrinsic_at_an_externally_sized_box() {
    let wide = "style='width:100px'";
    let mut doc = Document::builder(&page("text", wide), "").build().unwrap();
    doc.update(&page("longer text", wide), "").unwrap();
    let now = clock(&doc);
    assert_eq!(epochs(&doc, "p").layout, now);
    assert_eq!(epochs(&doc, "wrap").intrinsic, now);
    assert_ne!(
        epochs(&doc, "float").intrinsic,
        now,
        "px width stops the climb"
    );
    assert_eq!(epochs(&doc, "root").child, now);

    let mut doc = Document::builder(&page("text", ""), "").build().unwrap();
    doc.update(&page("longer text", ""), "").unwrap();
    let now = clock(&doc);
    assert_eq!(epochs(&doc, "float").intrinsic, now, "width:auto climbs");
}

#[test]
fn a_cell_with_a_length_width_still_passes_its_content_up() {
    let table = |t: &str| {
        format!(
            "<table id=t><tr id=r><td id=c style='width:100px'><span>{t}</span></td></tr></table>"
        )
    };
    let mut doc = Document::builder(&table("a"), "").build().unwrap();
    doc.update(&table("a much longer cell text"), "").unwrap();
    let now = clock(&doc);
    assert_eq!(epochs(&doc, "c").intrinsic, now);
    assert_eq!(
        epochs(&doc, "r").intrinsic,
        now,
        "td contribution reads content"
    );
}

#[test]
fn links_walk_parents_and_children_without_allocating_state() {
    let doc = Document::builder(&page("text", ""), "").build().unwrap();
    let tree = doc.state.boxes.borrow();
    let root = box_of(&doc, "root");
    let kids: Vec<_> = tree.children(root).collect();
    assert_eq!(kids, [box_of(&doc, "float"), box_of(&doc, "side")]);
    let up: Vec<_> = tree.ancestors(box_of(&doc, "p")).collect();
    assert!(up.starts_with(&[box_of(&doc, "wrap"), box_of(&doc, "float"), root]));
    assert!(up.iter().all(|&id| tree.get(id).is_some()));
}

#[test]
fn the_slot_lives_with_the_box_across_edits() {
    let mut doc = Document::builder(&page("text", ""), "").build().unwrap();
    let wrap = box_of(&doc, "wrap");
    assert!(doc.state.boxes.borrow().slot(wrap).is_none());
    BoxTree::edit(&doc.state.boxes)
        .slot_mut(wrap)
        .expect("live");
    doc.update(&page("text", "style='width:50px'"), "").unwrap();
    assert_eq!(box_of(&doc, "wrap"), wrap);
    assert!(doc.state.boxes.borrow().slot(wrap).is_some());
}

#[test]
fn clock_overflow_marks_every_box_changed() {
    let mut tree = BoxTree::default();
    tree.publish(&[]);
    tree.clock = u32::MAX;
    let next = tree.tick();
    assert_eq!(next, 2);
    assert!(tree.iter().all(|(_, r)| r.epochs.layout == 1));
}
