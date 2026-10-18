//! Shadow colors resolve at the affected fragment, after inheritance.

use super::*;
use crate::style::values::value::Color;

#[test]
fn inherited_currentcolor_uses_the_descendant_color() {
    let mut parent = Computed::default();
    parent.apply_one("color", "red");
    parent.apply_one("text-shadow", "2px 3px, 4px 5px blue");
    let mut own = Computed::default();
    own.apply_one("color", "green");
    let child = crate::style::cascade::inherit::inherit(&parent, &own);
    let shadows = shadow_list(&child);
    assert_eq!(shadows.len(), 2);
    assert_eq!(shadows[0].color, Color::parse("green").unwrap());
    assert_eq!((shadows[0].x, shadows[0].y), (2.0, 3.0));
    assert_eq!(shadows[1].color, Color::parse("blue").unwrap());
    assert!(child.text_shadow.unwrap().color.a < 0.0);
}

#[test]
fn explicit_currentcolor_defaults_to_black() {
    let mut style = Computed::default();
    style.apply_one("text-shadow", "1px 2px currentcolor");
    assert_eq!(shadow_list(&style)[0].color, Color::parse("black").unwrap());
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
    let mut parent = Computed::default();
    parent.apply_one("color", "red");
    parent.apply_one("text-shadow", "2px 3px");
    let mut green = Computed::default();
    green.apply_one("color", "green");
    let mut none = Computed::default();
    none.apply_one("text-shadow", "none");
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
    collect_groups(&nodes, &ShadowStyle::new(&parent), &mut groups);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0][0].color, Color::parse("red").unwrap());
    assert_eq!(groups[1][0].color, Color::parse("green").unwrap());
    assert!(parent.text_shadow.unwrap().color.a < 0.0);
}

#[test]
fn mask_keeps_decoration_records_and_original_text_color() {
    let mut owner = Computed::default();
    owner.apply_one("color", "red");
    owner.apply_one("text-decoration", "underline blue");
    owner.apply_one("text-shadow", "2px 3px green");
    let owner = crate::style::cascade::inherit::inherit(&Computed::default(), &owner);
    let context = ShadowStyle::new(&owner);
    let group = context.resolved();
    let nodes = vec![Node::Element(span(
        owner.clone(),
        vec![Node::Text("x".into())],
    ))];
    let masked = masks::mask_nodes(&nodes, &context, &group, &group[0]);
    let Node::Element(mask) = &masked[0] else {
        panic!("missing span")
    };
    assert_eq!(mask.style.color, owner.color);
    assert_eq!(mask.style.decors, owner.decors);
    assert_eq!(
        mask.style.text_shadow_ink,
        Some(Color::parse("green").unwrap())
    );
}

#[test]
fn text_only_preserves_shadow_ink_without_changing_computed_color() {
    let mut style = Computed::default();
    style.apply_one("color", "red");
    style.text_shadow_ink = Some(Color::parse("green").unwrap());
    let text = style.text_only();
    assert_eq!(text.color, style.color);
    assert_eq!(text.text_shadow_ink, style.text_shadow_ink);
}
