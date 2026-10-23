//! Тесты `text-shadow`: `currentColor` тени решается цветом затронутого фрагмента после
//! наследования (css-text-decor-3 §4), `none` и вложенные цвета дают независимые группы, маска
//! тени сохраняет записи декораций и исходный цвет текста.

use super::*;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Color;

#[test]
fn inherited_currentcolor_uses_the_descendant_color() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut parent = Computed::default();
    parent.apply_one("color", "red", vcx);
    parent.apply_one("text-shadow", "2px 3px, 4px 5px blue", vcx);
    let mut own = Computed::default();
    own.apply_one("color", "green", vcx);
    let child = crate::style::cascade::inherit::inherit(&BoxStyle::from(parent.clone()), &own, vcx);
    let shadows = shadow_list(&child);
    assert_eq!(shadows.len(), 2);
    assert_eq!(shadows[0].color, Color::parse("green", vcx).unwrap().into());
    assert_eq!((shadows[0].x, shadows[0].y), (2.0, 3.0));
    assert_eq!(shadows[1].color, Color::parse("blue", vcx).unwrap().into());
    assert!(child.text_shadow.clone().unwrap().color.is_current());
}

#[test]
fn explicit_currentcolor_defaults_to_black() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut style = Computed::default();
    style.apply_one("text-shadow", "1px 2px currentcolor", vcx);
    assert_eq!(
        shadow_list(&style)[0].color,
        Color::parse("black", vcx).unwrap().into()
    );
}

fn span(style: Computed, children: Vec<Node>) -> crate::dom::Element {
    crate::dom::Element {
        marks: Default::default(),
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "span".into(),
        style: style.into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children,
        attrs: Vec::new(),
        inline: true,
    }
}

#[test]
fn nested_currentcolor_and_none_keep_independent_shadow_groups() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut parent = Computed::default();
    parent.apply_one("color", "red", vcx);
    parent.apply_one("text-shadow", "2px 3px", vcx);
    let mut green = Computed::default();
    green.apply_one("color", "green", vcx);
    let mut none = Computed::default();
    none.apply_one("text-shadow", "none", vcx);
    let nodes = vec![
        Node::Text("red".into()),
        Node::Element(span(
            green,
            vec![
                Node::Text("green".into()),
                Node::Element(span(none, vec![Node::Text("none".into())])),
            ],
        )),
    ];
    let mut groups = Vec::new();
    collect_groups(&nodes, &ShadowStyle::new(&parent), &mut groups, vcx);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0][0].color, Color::parse("red", vcx).unwrap().into());
    assert_eq!(
        groups[1][0].color,
        Color::parse("green", vcx).unwrap().into()
    );
    assert!(parent.text_shadow.clone().unwrap().color.is_current());
}

#[test]
fn mask_keeps_decoration_records_and_original_text_color() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut owner = Computed::default();
    owner.apply_one("color", "red", vcx);
    owner.apply_one("text-decoration", "underline blue", vcx);
    owner.apply_one("text-shadow", "2px 3px green", vcx);
    let owner = crate::style::cascade::inherit::inherit(&BoxStyle::default(), &owner, vcx);
    let context = ShadowStyle::new(&owner);
    let group = context.resolved();
    let nodes = vec![Node::Element(span(
        (owner.clone()).into_computed(),
        vec![Node::Text("x".into())],
    ))];
    let masked = masks::mask_nodes(&nodes, &context, &group, &group[0], vcx);
    let Node::Element(mask) = &masked[0] else {
        panic!("missing span")
    };
    assert_eq!(mask.style.cascaded().color, owner.color);
    assert_eq!(mask.style.used().decors, owner.decors);
    assert_eq!(
        mask.style.used().text_shadow_ink,
        Some(Color::parse("green", vcx).unwrap())
    );
}

#[test]
fn text_only_preserves_shadow_ink_without_changing_computed_color() {
    let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
    let vcx = doc.values();
    let mut style = Computed::default();
    style.apply_one("color", "red", vcx);
    style.text_shadow_ink = Some(Color::parse("green", vcx).unwrap());
    let text = style.text_only();
    assert_eq!(text.color, style.color);
    assert_eq!(text.text_shadow_ink, style.text_shadow_ink);
}
