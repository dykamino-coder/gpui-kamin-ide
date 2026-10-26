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
    let section = element(&parsed.nodes, "section").style.published().unwrap();
    let i = find(&parsed.nodes, "i").unwrap();
    let computed = i.style.published().unwrap();
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
    let held = b.style.published().unwrap();
    assert_eq!(held.padding.left, Some(Len::Px(5.0)));
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    assert!(b.style.published().unwrap().ptr_eq(&held));
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
    // Копия родительской коробки не пересчитывает стиль элемента.
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

#[test]
fn used_inherited_values_of_layout_copies_do_not_reach_descendants() {
    let parsed = crate::dom::parse(
        r#"<body><div style="font-size:20px;line-height:2;text-wrap:balance;ruby-align:center;list-style-type:square;list-style-position:inside"><span><i>x</i></span></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = element(&parsed.nodes, "div");
    let span = element(&parsed.nodes, "span");
    let i = element(&parsed.nodes, "i");
    let mut copy = div.clone();
    // Руби, отдельная область float и строчный split меняют только свою коробку.
    copy.style.used_mut().inh = Computed::default().inh;
    let parent = box_style(&BoxStyle::default(), &copy.style, vcx);
    let child = box_style(&parent, &span.style, vcx);
    let grandchild = box_style(&child, &i.style, vcx);
    assert_eq!(child.inh, span.style.computed().inh);
    assert_eq!(grandchild.inh, i.style.computed().inh);
    assert!(Rc::ptr_eq(
        &child.element_style.as_ref().unwrap().computed,
        &span.style.computed()
    ));
}

#[test]
fn vertical_font_units_use_the_elements_axes_under_an_upright_stack_copy() {
    for mode in ["vertical-rl", "vertical-lr"] {
        let parsed = crate::dom::parse(
            &format!(
                r#"<body><div style="font-size:20px;writing-mode:{mode};text-orientation:upright"><span style="position:absolute;height:2ch;width:3ic">x</span></div></body>"#,
            ),
            "",
        );
        let vcx = parsed.values();
        materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
        let div = element(&parsed.nodes, "div");
        let span = element(&parsed.nodes, "span");
        let computed = span.style.computed();
        let mut stack = box_style(&BoxStyle::default(), &div.style, vcx);
        stack.writing_mode = None;
        stack.line_height = Some(Len::Px(20.0));
        let positioned = box_style(&stack, &span.style, vcx);
        assert_eq!(computed.height, Some(Len::Px(40.0)));
        assert_eq!(computed.width, Some(Len::Px(60.0)));
        assert_eq!(positioned.height, computed.height);
        assert_eq!(positioned.width, computed.width);
        assert_eq!(positioned.wm(), computed.wm());
        assert_eq!(div.style.computed().wm(), computed.wm());
    }
}

#[test]
fn a_computed_cascade_variant_reaches_descendants_without_used_values() {
    let parsed = crate::dom::parse(
        r#"<body><div style="width:10px;font-size:20px"><span style="width:inherit"><i style="width:inherit">x</i></span></div></body>"#,
        "",
    );
    let vcx = parsed.values();
    materialize(&parsed.nodes, &Rc::new(Computed::default()), vcx);
    let div = element(&parsed.nodes, "div");
    let mut parent = box_style(&BoxStyle::default(), &div.style, vcx);
    assert!(recompute_onto(
        &mut parent,
        &div.style,
        |c| c.width = Some(Len::Px(30.0)),
        vcx
    ));
    parent.width = Some(Len::Px(99.0));
    parent.font_size = Some(Len::Px(7.0));
    let child = box_style(&parent, &element(&parsed.nodes, "span").style, vcx);
    let grandchild = box_style(&child, &element(&parsed.nodes, "i").style, vcx);
    assert_eq!(child.width, Some(Len::Px(30.0)));
    assert_eq!(grandchild.width, Some(Len::Px(30.0)));
    assert_eq!(child.font_size, Some(Len::Px(20.0)));
    assert_eq!(grandchild.font_size, Some(Len::Px(20.0)));
    assert_eq!(div.style.computed().width, Some(Len::Px(10.0)));
}
