//! Тесты досягаемости позиционированных потомков (`oof_reach`): абсолют сквозь статическую
//! обёртку не двигает братьев, заданный `top` считается от непрерывного начала содержащего
//! блока в колонках, `fixed` внутри `contain: layout` доходит до своего содержащего блока.

use super::*;
use crate::dom::{Element, Node};
use crate::layout::fragment::fragment_size::shape_full;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

fn element(style: Computed, children: Vec<Node>) -> Element {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    Element {
        marks: Default::default(),
        list_item: None,
        node_id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        anim: None,
        tag: "div".into(),
        style: style.into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children,
        attrs: Vec::new(),
        inline: false,
    }
}

#[test]
fn positioned_reach_crosses_static_wrappers_without_moving_siblings() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    use crate::style::computed::Position;
    let abs = element(
        Computed {
            height: Some(Len::Px(200.0)),
            position: Some(Position::Absolute),
            ..Default::default()
        },
        vec![],
    );
    let wrapper = element(Computed::default(), vec![Node::Element(abs)]);
    let sibling = element(
        Computed {
            height: Some(Len::Px(20.0)),
            ..Default::default()
        },
        vec![],
    );
    let parent = element(
        Computed {
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(wrapper), Node::Element(sibling)],
    );
    let m =
        crate::layout::fragment::fragment_size::measure(&parent, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert_eq!(m.shape.height, 200.0);
    assert_eq!(m.own, Some((20.0, 200.0)));
}

#[test]
fn explicit_top_uses_continuous_containing_block_origin_in_columns() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    use crate::style::computed::Position;
    let first = element(
        Computed {
            height: Some(Len::Px(100.0)),
            ..Default::default()
        },
        vec![],
    );
    let mut abs = element(
        Computed {
            height: Some(Len::Px(200.0)),
            position: Some(Position::Absolute),
            ..Default::default()
        },
        vec![],
    );
    abs.style.cascaded_mut().inset.top = Some(Len::Px(0.0));
    let parent = element(
        Computed {
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(first), Node::Element(abs)],
    );
    assert_eq!(
        shape_full(&parent, 4, ShapeCx::COLUMNS, vcx)
            .unwrap()
            .height,
        200.0
    );
}

#[test]
fn contained_fixed_reach_passes_relative_boxes_to_its_own_cb() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    use crate::style::computed::Position;
    let mut fixed = element(
        Computed {
            height: Some(Len::Px(200.0)),
            position: Some(Position::Fixed),
            ..Default::default()
        },
        vec![],
    );
    fixed.style.cascaded_mut().inset.top = Some(Len::Px(0.0));
    let relative = element(
        Computed {
            position: Some(Position::Relative),
            ..Default::default()
        },
        vec![Node::Element(fixed)],
    );
    let parent = element(
        Computed {
            contain_layout: Some(true),
            ..Default::default()
        },
        vec![Node::Element(relative)],
    );
    assert_eq!(
        shape_full(&parent, 4, ShapeCx::COLUMNS, vcx)
            .unwrap()
            .height,
        200.0
    );
}

#[test]
fn measure_hands_the_positioned_reach_up_as_a_value() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    use crate::layout::fragment::fragment_size::measure;
    use crate::style::computed::Position;
    let abs = element(
        Computed {
            height: Some(Len::Px(150.0)),
            position: Some(Position::Absolute),
            ..Default::default()
        },
        vec![],
    );
    // The wrapper is not a containing block: the reach travels up from it.
    let wrapper = element(Computed::default(), vec![Node::Element(abs)]);
    let m = measure(&wrapper, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert_eq!(m.shape.height, 0.0);
    // Measuring an unrelated box in between cannot change a returned value
    // (it used to be a slot keyed by node id).
    let other = element(
        Computed {
            height: Some(Len::Px(10.0)),
            ..Default::default()
        },
        vec![],
    );
    let none = measure(&other, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert_eq!(none.up.absolute.end(), 0.0);
    assert_eq!(m.up.absolute.end(), 150.0);
}
