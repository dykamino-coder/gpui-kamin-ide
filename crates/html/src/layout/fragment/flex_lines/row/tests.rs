//! Тесты меры многострочного flex-ряда (`measure_rows`) против css-flexbox-1 §9.3/§9.7/§12:
//! высота строки — наибольшая внешняя высота, вложенный ряд сохраняет границу разрыва,
//! растянутый `avoid`-элемент сплошной на всю строку, строки собираются до сжатия,
//! разрыв на краю доходит до контейнера, разорванный `row-gap` не растит следующую строку.

use crate::dom::{Element, Node};
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::fragment_size::shape_full;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

fn element(style: Computed, children: Vec<Element>) -> Element {
    Element {
        marks: Default::default(),
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "div".into(),
        style: style.into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: children.into_iter().map(Node::Element).collect(),
        attrs: Vec::new(),
        inline: false,
    }
}

fn item(width: f32, height: f32) -> Element {
    element(
        Computed {
            width: Some(Len::Px(width)),
            height: Some(Len::Px(height)),
            ..Default::default()
        },
        Vec::new(),
    )
}

fn rows(width: Option<f32>, children: Vec<Element>) -> Element {
    element(
        Computed {
            display: Some(Display::Flex),
            flex_wrap: Some(true),
            width: width.map(Len::Px),
            ..Default::default()
        },
        children,
    )
}

#[test]
fn row_height_is_the_maximum_outer_item_height() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let c = rows(
        Some(50.0),
        vec![item(25.0, 20.0), item(25.0, 80.0), item(25.0, 60.0)],
    );
    let shape = shape_full(&c, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert_eq!(shape.height, 140.0);
    assert!(shape.cuts.contains(&(80.0, 80.0)));
    assert!(!shape.cuts.contains(&(20.0, 20.0)));
}

#[test]
fn nested_auto_width_rows_keep_their_break_boundary() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let first = rows(None, vec![item(25.0, 100.0), item(25.0, 100.0)]);
    let mut second = first.clone();
    second.style.used_mut().break_before_force = true;
    let c = rows(Some(50.0), vec![first, second]);
    let shape = shape_full(&c, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert_eq!(shape.height, 200.0);
    assert_eq!(shape.forced, vec![100.0]);
}

#[test]
fn stretched_avoid_item_is_solid_for_the_whole_row() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut empty = item(25.0, 0.0);
    empty.style.used_mut().height = None;
    empty.style.used_mut().break_inside_avoid = true;
    let c = rows(Some(50.0), vec![item(25.0, 80.0), empty]);
    let shape = shape_full(&c, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert_eq!(shape.height, 80.0);
    assert!(shape.solid.contains(&(0.0, 80.0)));
}

#[test]
fn hypothetical_size_collects_lines_before_shrink() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let nested = rows(None, vec![item(50.0, 25.0), item(50.0, 25.0)]);
    let c = rows(Some(50.0), vec![nested, item(0.0, 200.0)]);
    let shape = shape_full(&c, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert_eq!(shape.height, 250.0);
    assert!(shape.cuts.contains(&(50.0, 50.0)));
}

#[test]
fn break_before_on_the_second_item_of_the_first_row_reaches_the_container() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut second = item(25.0, 25.0);
    second.style.used_mut().break_before_force = true;
    let c = rows(Some(50.0), vec![item(25.0, 25.0), second, item(50.0, 25.0)]);
    assert!(crate::layout::fragment::breaks::edge_break(&c, false, vcx));
    assert!(!crate::layout::fragment::breaks::edge_break(&c, true, vcx));
}

#[test]
fn a_fragmented_row_gap_does_not_expand_the_next_row() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut c = rows(Some(50.0), vec![item(50.0, 50.0), item(50.0, 50.0)]);
    c.style.used_mut().gap = Some((Some(Len::Px(100.0)), None));
    let shape = shape_full(&c, 4, ShapeCx::COLUMNS, vcx).unwrap();
    assert!(shape.cuts.contains(&(50.0, 150.0)));
    let lead = item(50.0, 25.0);
    let lead_shape = shape_full(&lead, 4, ShapeCx::COLUMNS, vcx).unwrap();
    let grown = crate::layout::fragment::push::grow_pushed(
        vec![(lead, lead_shape), (c, shape)],
        3,
        Some(100.0),
        None,
        3,
        &[Default::default(); 2],
        &|_| false,
        vcx,
    );
    assert_eq!(grown[1].1.height, 200.0);
    assert!(
        grown[1]
            .0
            .children
            .iter()
            .all(|n| { matches!(n, Node::Element(k) if k.style.used().margin.top.is_none()) })
    );
}
