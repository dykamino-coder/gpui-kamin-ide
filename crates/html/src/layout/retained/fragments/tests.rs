//! Inline geometry and the lifetime of a kept paragraph's document observers.

use super::*;

fn painted(stamp: u64) -> InlineFragments {
    let mut f = InlineFragments::default();
    f.begin(stamp, (10., 110.), 2., 1.);
    f.push(7, 0., 20.);
    f.push(7, 20., 40.);
    f.push(8, 20., 40.);
    f
}

#[test]
fn a_box_over_two_lines_has_two_line_rects() {
    let f = painted(1);
    let rects: Vec<_> = f.rects(7).collect();
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[1].origin, point(px(10.), px(20.)));
    assert_eq!(rects[1].size, size(px(100.), px(20.)));
    assert!(!f.has(9));
}

#[test]
fn the_ink_grows_by_the_bleed_and_the_outset() {
    let ink = painted(1).ink(8, 3.).unwrap();
    assert_eq!(ink.origin, point(ScaledPixels(5.), ScaledPixels(15.)));
    assert_eq!(ink.size, size(ScaledPixels(110.), ScaledPixels(30.)));
    assert!(painted(1).ink(9, 0.).is_none());
}

#[test]
fn a_new_frame_replaces_and_the_same_frame_adds() {
    let mut f = painted(1);
    f.begin(1, (0., 50.), 1., 1.);
    f.push(9, 40., 60.);
    assert!(f.has(7) && f.has(9));
    assert_eq!(f.rects(9).next().unwrap().origin.x, px(0.));
    f.begin(2, (0., 50.), 1., 1.);
    f.push(9, 0., 20.);
    assert!(!f.has(7));
    assert_eq!(f.stamp(), 2);
}

fn target(boxes: &std::rc::Rc<RefCell<BoxTree>>, store: &std::rc::Rc<Store>) -> FragmentTarget {
    FragmentTarget {
        boxes: std::rc::Rc::downgrade(boxes),
        owner: boxes
            .borrow()
            .iter()
            .next()
            .map_or(BoxId::NONE, |(id, _)| id),
        store: std::rc::Rc::downgrade(store),
    }
}

#[test]
fn a_kept_paragraph_does_not_keep_its_document_owners_alive() {
    use crate::text::paragraph::{KeptAtoms, KeptSlot, Paragraph};
    use std::rc::Rc;

    let boxes = Rc::new(RefCell::new(BoxTree::default()));
    let store = Rc::new(Store::default());
    let weak_boxes = Rc::downgrade(&boxes);
    let weak_store = Rc::downgrade(&store);
    let slot = Rc::new(KeptSlot::default());
    let expired = target(&boxes, &store);
    let mut para = Paragraph::empty().fragments_to(Some(target(&boxes, &store)));
    assert!(para.keep_into(slot.clone(), KeptAtoms::default()));
    para.keep_painted();
    assert!(slot.is_filled());
    drop((boxes, store));
    assert!(
        weak_boxes.upgrade().is_none(),
        "kept geometry retained the box tree"
    );
    assert!(
        weak_store.upgrade().is_none(),
        "kept geometry retained its owning Store"
    );
    expired.write((0., 10.), 0., 1., |_| {
        panic!("an expired target must not write geometry");
    });
}

#[test]
fn a_live_document_still_receives_fragments_after_a_borrow_conflict() {
    use crate::document::Document;
    use std::rc::Rc;

    let doc = Document::builder("<p>live</p>", "").build().unwrap();
    let boxes = doc.state_rc().boxes.clone();
    let store = Rc::new(Store::default());
    let target = target(&boxes, &store);
    let held = boxes.borrow();
    target.write((10., 110.), 2., 1., |_| panic!("tree is borrowed"));
    drop(held);
    target.write((10., 110.), 2., 1., |fragments| fragments.push(7, 0., 20.));
    let boxes = boxes.borrow();
    let fragments = boxes.slot(target.owner).unwrap().inline.as_ref().unwrap();
    assert_eq!(fragments.stamp(), store.frame_epoch());
    assert_eq!(
        fragments.rects(7).next().unwrap().size,
        size(px(100.), px(20.))
    );
}

#[test]
fn the_lines_ink_bounds_every_line_culled_text_too() {
    let mut f = InlineFragments::default();
    assert!(f.lines_ink(0.).is_none());
    f.begin(1, (10., 110.), 2., 1.);
    // A paragraph of culled text only: no fragment, its lines all the same.
    f.push(0, 40., 60.);
    f.push(0, 60., 80.);
    assert!(!f.has(0));
    // Another paragraph of the same frame adds its lines.
    f.begin(1, (0., 50.), 1., 1.);
    f.push(7, 0., 20.);
    let ink = f.lines_ink(1.).unwrap();
    assert_eq!(ink.origin, point(ScaledPixels(-3.), ScaledPixels(-3.)));
    assert_eq!(ink.size, size(ScaledPixels(116.), ScaledPixels(86.)));
    // A new frame starts the lines again.
    f.begin(2, (0., 50.), 0., 1.);
    assert!(f.lines_ink(0.).is_none());
}