//! Доли детей ячейки с заданной высотой: что решается заранее, что остаётся раскладке.

use super::resolve_child_percents;
use crate::dom::Node;
use crate::layout::table::anon::anon_element;
use crate::style::computed::Overflow;
use crate::style::values::value::Len;

fn kid(tag: &str, height: Len) -> Node {
    let mut el = anon_element(tag, vec![]);
    el.style.used_mut().reset_box_layout.height = Some(height);
    Node::Element(el)
}

fn height(n: &Node) -> Option<Len> {
    match n {
        Node::Element(el) => el.style.used().reset_box_layout.height,
        _ => None,
    }
}

#[test]
fn plain_percentage_child_is_left_to_the_cell_final_height() {
    let mut cell = anon_element("td", vec![kid("div", Len::Pct(2.0))]);
    resolve_child_percents(&mut cell, 200.0);
    assert_eq!(height(&cell.children[0]), Some(Len::Pct(2.0)));
}

#[test]
fn replaced_and_scrollable_children_resolve_against_the_specified_height() {
    let mut scroller = kid("div", Len::Pct(1.0));
    if let Node::Element(el) = &mut scroller {
        el.style.used_mut().reset_box_layout.overflow_y = Some(Overflow::Scroll);
    }
    let mut cell = anon_element("td", vec![kid("canvas", Len::Pct(2.0)), scroller]);
    resolve_child_percents(&mut cell, 50.0);
    assert_eq!(height(&cell.children[0]), Some(Len::Px(100.0)));
    assert_eq!(height(&cell.children[1]), Some(Len::Px(50.0)));
}
