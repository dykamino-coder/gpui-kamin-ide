//! Распространение разрыва с первой строки многострочного flex-ряда на контейнер
//! (css-flexbox-1 §10): `break-before` второго элемента первой строки доходит до края.

use crate::dom::{Element, Node};
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
        children: (children.into_iter().map(Node::Element).collect::<Vec<_>>()).into(),
        attrs: Vec::new(),
        inline: false,
    }
}

fn item(width: f32, height: f32) -> Element {
    element(
        Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    width: Some(Len::Px(width)),
                    height: Some(Len::Px(height)),
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
        Vec::new(),
    )
}

#[test]
fn break_before_on_the_second_item_of_the_first_row_reaches_the_container() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = crate::layout::cx::FrameCx::new(&doc);
    let mut second = item(25.0, 25.0);
    second
        .style
        .used_mut()
        .reset_fragmentation
        .break_before_force = true;
    let c = element(
        Computed {
            reset_box_layout: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetBoxLayout {
                    display: Some(Display::Flex),
                    width: Some(Len::Px(50.0)),
                    ..Default::default()
                },
            ),
            reset_flex: crate::style::computed::StyleGroup::new(
                crate::style::computed::ResetFlex {
                    flex_wrap: Some(true),
                    ..Default::default()
                },
            ),
            ..Default::default()
        },
        vec![item(25.0, 25.0), second, item(50.0, 25.0)],
    );
    assert!(crate::layout::fragment::breaks::edge_break(&c, false, vcx));
    assert!(!crate::layout::fragment::breaks::edge_break(&c, true, vcx));
}
