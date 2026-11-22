//! Classic bar ownership and the measured range against bars layout decided.

use super::*;
use crate::layout::table::anon::anon_element;
use crate::style::computed::{ResetBoxLayout, StyleGroup};

fn scrolling(overflow: Overflow) -> Computed {
    Computed {
        reset_box_layout: StyleGroup::new(ResetBoxLayout {
            overflow_x: Some(overflow),
            overflow_y: Some(overflow),
            ..Default::default()
        }),
        ..Computed::default()
    }
}

/// The root frame with a measured vertical range, as `classic::update`
/// leaves it after a settled frame.
fn settled(owner: &Element) -> Frame {
    let frame = Frame::default().begin(Some(owner)).unwrap();
    frame.range.set([0.0, 500.0]);
    assert!(update(&frame, &scrolling(Overflow::Scroll), [400.0, 300.0]));
    frame
}

#[test]
fn anonymous_boxes_keep_the_viewport_bars() {
    let viewport = anon_element("html-viewport", vec![]);
    let frame = settled(&viewport);
    let style = scrolling(Overflow::Scroll);
    assert_eq!(model(Some(&frame), &viewport, &style).bars, [false, true]);
    // An anonymous table box and a pseudo-element share the viewport's id 0.
    for tag in ["tr", "::before"] {
        let anon = anon_element(tag, vec![]);
        assert_eq!(
            model(Some(&frame), &anon, &Computed::default()).bars,
            [false; 2]
        );
        assert_eq!(model(Some(&frame), &anon, &style).bars, [false; 2]);
    }
    assert_eq!(
        frame.bars(),
        [false, true],
        "a box that is not the owner reset the bars"
    );
    assert!(
        !update(&frame, &style, [400.0, 300.0]),
        "the next frame sees new bars"
    );
}

#[test]
fn an_owner_that_stops_scrolling_writes_nothing() {
    let mut owner = anon_element("div", vec![]);
    owner.node_id = 7;
    let frame = settled(&owner);
    assert_eq!(
        model(Some(&frame), &owner, &Computed::default()).bars,
        [false; 2]
    );
    assert_eq!(frame.bars(), [false, true]);
    assert_eq!(
        model(Some(&frame), &owner, &scrolling(Overflow::Hidden)).bars,
        [false; 2]
    );
    assert_eq!(
        frame.bars(),
        [false; 2],
        "a hidden scroll container has no bars"
    );
}
#[test]
fn a_bar_layout_decided_holds_against_the_measured_range() {
    // Layout decided both bars of an `auto` scrollport; its measured range
    // finds only the vertical one. The range checks, it does not lay the box
    // out again: a disagreement repeated every frame never goes quiet.
    let mut owner = anon_element("div", vec![]);
    owner.node_id = 7;
    let frame = Frame::default().begin(Some(&owner)).unwrap();
    let style = scrolling(Overflow::Scroll);
    frame.settling.decided.set([true, true]);
    assert!(adopt(&frame, &style, [true, true]));
    frame.range.set([0.0, 15.0]);
    assert!(
        !update(&frame, &style, [751.0, 124.0]),
        "the range changed bars layout decided"
    );
    assert_eq!(frame.bars(), [true, true]);
    // An axis layout does not decide still takes the measured range.
    frame.settling.decided.set([true, false]);
    frame.range.set([0.0, 0.0]);
    assert!(update(&frame, &style, [751.0, 124.0]));
    assert_eq!(frame.bars(), [true, false]);
}
