//! Материализация идёт по дереву элементов, а не по перестроенному дереву коробок: анонимная
//! обёртка и перенос коробки родителя-элемента не меняют.

use super::tests::{element, find};
use super::*;
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

#[test]
fn a_moved_grid_descendant_inherits_from_its_parent_element() {
    let parsed = crate::dom::parse(
        r#"<body><div style="display: grid; position: relative; font-size: 10px; color: red; padding-left: 3px"><section style="font-size: 20px; color: blue; padding-left: 7px"><i style="position: absolute; grid-column: 1; padding-left: inherit">x</i></section></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    let grid = element(&parsed.nodes, "div");
    // The box moved under the grid (its containing block), the element did not.
    assert!(
        grid.children
            .iter()
            .any(|n| matches!(n, Node::Element(e) if e.tag == "i"))
    );
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let section = element(&parsed.nodes, "section").style.computed();
    let i = find(&parsed.nodes, "i").unwrap();
    let computed = i.style.computed();
    assert_eq!(computed.font_size, section.font_size);
    assert_eq!(computed.color, section.color);
    assert_eq!(computed.padding.left, Some(Len::Px(7.0)));
    // The box under the grid's box keeps the element parent's values.
    let boxed = box_style(&BoxStyle::shared(grid.style.computed()), &i.style, vcx);
    assert_eq!(boxed.font_size, section.font_size);
    assert_eq!(boxed.color, section.color);
    assert_eq!(boxed.padding.left, Some(Len::Px(7.0)));
}

#[test]
fn an_anonymous_table_passes_its_parent_element_through() {
    let parsed = crate::dom::parse(
        r#"<body><span style="padding-left: 5px">a<b style="display: table-cell; padding-left: inherit">c</b></span></body>"#,
        "",
    );
    let vcx = parsed.values();
    let span = element(&parsed.nodes, "span");
    assert!(
        span.children
            .iter()
            .any(|n| matches!(n, Node::Element(e) if e.marks.anonymous_wrapper))
    );
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let b = find(&parsed.nodes, "b").unwrap();
    // `inherit` takes the value of the parent element, not of the anonymous table.
    assert_eq!(b.style.computed().padding.left, Some(Len::Px(5.0)));
}

#[test]
fn a_box_under_another_parent_keeps_the_elements_perspective_cell() {
    let parsed = crate::dom::parse(
        r#"<body><div style="perspective: 50px"><section style="perspective: inherit; zoom: 2"><i>x</i></section></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = element(&parsed.nodes, "div");
    let section = element(&parsed.nodes, "section");
    // A box parent with other values (a layout copy) recomputes the box.
    let mut parent = box_style(
        &BoxStyle::shared(Rc::new(Computed::default())),
        &div.style,
        vcx,
    );
    parent.width = Some(Len::Px(7.0));
    let boxed = box_style(&parent, &section.style, vcx);
    assert_eq!(boxed.perspective, Some(100.0));
    let (Some(cell), Some(own)) = (
        boxed.perspective_frame.as_ref(),
        section.style.computed().perspective_frame.clone(),
    ) else {
        panic!("perspective cells")
    };
    assert!(Rc::ptr_eq(cell, &own));
}
