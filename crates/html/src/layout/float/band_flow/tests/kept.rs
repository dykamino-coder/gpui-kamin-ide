//! The host's plans between frames: valid while its subtree and outside input hold.

use super::super::{HostSlot, Plan, Slot};
use crate::document::Document;
use crate::layout::reuse::{BandBoxes, BandKey};

fn page(inner: &str, other: &str) -> String {
    format!(
        "<div id=host><div style='float:left'><p>{inner}</p></div><p>after</p></div>\
         <p id=other>{other}</p>"
    )
}

fn host(doc: &Document) -> HostSlot {
    let node = find_host(doc);
    let boxes = doc.boxes_for_tests().1;
    let own = boxes.borrow().box_of(node).expect("host box");
    let owner = BandBoxes {
        slot: own,
        epoch: own,
    };
    HostSlot::new(boxes, owner, BandKey::for_tests(800.0), true)
}

fn find(node: &crate::dom::Node, id: &str) -> Option<u64> {
    let crate::dom::Node::Element(e) = node else {
        return None;
    };
    if e.attrs.iter().any(|(n, v)| n == "id" && v == id) {
        return Some(e.node_id);
    }
    e.children.iter().find_map(|c| find(c, id))
}

fn plan(width: f32, height: f32) -> Plan {
    Plan {
        width,
        height,
        slots: vec![Slot {
            y: 3.0,
            kids: vec![Slot::default()],
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn a_kept_plan_holds_until_the_hosts_subtree_changes() {
    let mut doc = Document::builder(&page("text", "other"), "")
        .build()
        .unwrap();
    let slot = host(&doc);
    assert!(slot.plan(100.0).is_none());
    slot.keep_plan(&plan(100.0, 12.0));
    slot.keep_intrinsic((10.0, 90.0));
    let kept = slot.plan(100.0).expect("kept plan");
    assert_eq!((kept.height, kept.slots[0].y), (12.0, 3.0));
    assert_eq!(kept.slots[0].kids.len(), 1);
    assert!(slot.plan(50.0).is_none(), "another width is another input");
    assert_eq!(slot.intrinsic(), Some((10.0, 90.0)));

    // An edit outside the host leaves its result.
    doc.update(&page("text", "changed"), "").unwrap();
    assert_eq!(host(&doc).plan(100.0).map(|p| p.height), Some(12.0));

    // An edit inside it makes the result stale; a new one replaces it.
    doc.update(&page("longer text", "changed"), "").unwrap();
    let slot = host(&doc);
    assert!(slot.plan(100.0).is_none());
    assert!(slot.intrinsic().is_none());
    slot.keep_plan(&plan(100.0, 24.0));
    assert_eq!(slot.plan(100.0).map(|p| p.height), Some(24.0));
}

#[test]
fn another_outside_input_or_a_reading_ban_takes_no_kept_plan() {
    let doc = Document::builder(&page("text", "other"), "")
        .build()
        .unwrap();
    let slot = host(&doc);
    slot.keep_plan(&plan(100.0, 12.0));
    let boxes = doc.boxes_for_tests().1;
    let own = boxes.borrow().box_of(find_host(&doc)).unwrap();
    let owner = BandBoxes {
        slot: own,
        epoch: own,
    };
    let wider = HostSlot::new(boxes, owner, BandKey::for_tests(900.0), true);
    assert!(wider.plan(100.0).is_none());
    let banned = HostSlot::new(boxes, owner, BandKey::for_tests(800.0), false);
    assert!(banned.plan(100.0).is_none());
}

#[test]
fn the_kept_plans_are_bounded_and_drop_with_the_document() {
    let doc = Document::builder(&page("text", "other"), "")
        .build()
        .unwrap();
    let slot = host(&doc);
    for width in 0..20 {
        slot.keep_plan(&plan(width as f32, 1.0));
    }
    let kept = (0..20).filter(|w| slot.plan(*w as f32).is_some()).count();
    assert_eq!(kept, 4);
    drop(doc);
    assert!(
        slot.plan(19.0).is_none(),
        "the slot holds the box tree weakly"
    );
}

fn find_host(doc: &Document) -> u64 {
    doc.boxes_for_tests()
        .0
        .iter()
        .find_map(|n| find(n, "host"))
        .unwrap()
}
