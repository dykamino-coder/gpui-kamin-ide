//! Наследование всех вариантов list-style-type от элемента и локальность
//! содержимого ::marker: копии коробок не подменяют вычисленный источник.

use super::*;
use crate::document::state::DocState;
use crate::style::cascade::materialize::{box_style, materialize};
use crate::style::css::parse_decls;
use std::rc::Rc;

fn style(css: &str, vcx: ValueCx<'_>) -> Computed {
    let mut c = Computed::default();
    c.apply_decls_with_vars(&parse_decls(css), &Default::default(), vcx);
    c
}

#[test]
fn marker_type_is_inherited_through_elements_without_markers() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    for (parent, child, expected) in [
        ("'text '", "", Some("text ")),
        ("''", "", Some("")),
        ("'text '", "list-style-type:inherit", Some("text ")),
        ("'text '", "list-style-type:unset", Some("text ")),
        ("'text '", "list-style-type:decimal", Some("3. ")),
        ("'text '", "list-style-type:initial", Some("• ")),
        ("decimal", "list-style-type:'own '", Some("own ")),
        ("none", "", None),
        ("none", "list-style-type:decimal", Some("3. ")),
        ("'text '", "list-style-type:none", None),
    ] {
        let mut item = Element::synthetic(
            "li",
            style(&format!("display:list-item;{child}"), vcx),
            Default::default(),
            vec![],
        );
        item.list_item = Some(3);
        let wrapper = Element::synthetic(
            "div",
            Computed::default(),
            Default::default(),
            vec![Node::Element(item)],
        );
        let parent = Element::synthetic(
            "div",
            style(&format!("list-style-type:{parent}"), vcx),
            Default::default(),
            vec![Node::Element(wrapper)],
        );
        let mut nodes = [Node::Element(parent)];
        materialize(&nodes, &Rc::new(Computed::default()), vcx);
        let Node::Element(parent) = &mut nodes[0] else {
            unreachable!()
        };
        parent.style.used_mut().list_style_type = Some(MarkerType::Text("layout ".into()));
        let Node::Element(wrapper) = &parent.children[0] else {
            unreachable!()
        };
        let Node::Element(item) = &wrapper.children[0] else {
            unreachable!()
        };
        let boxed = box_style(&BoxStyle::default(), &item.style, vcx);
        let marker = resolve(item, &Computed::default(), &boxed, vcx);
        assert_eq!(
            marker.as_ref().map(|m| m.text.as_str()),
            expected,
            "{child}"
        );
    }
}

#[test]
fn local_marker_content_and_suppression_are_not_inherited() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    let mut parent = style("list-style-type:'inherited '", vcx);
    parent.marker_text = Some("local ".into());
    parent.no_marker = Some(true);
    let item = Element::synthetic(
        "li",
        style("display:list-item", vcx),
        Default::default(),
        vec![],
    );
    let merged = inherit(&BoxStyle::new(parent), &item.style.used(), vcx);
    assert_eq!(merged.marker_text, None);
    assert_eq!(merged.no_marker, None);
    assert_eq!(
        resolve(&item, &Computed::default(), &merged, vcx)
            .unwrap()
            .text,
        "inherited "
    );
}

#[test]
fn only_outside_generated_symbols_discard_text_spacing() {
    use crate::style::values::value::Len;
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    for (css, symbol) in [
        ("list-style:disc outside", true),
        ("list-style:square outside", true),
        ("list-style:disc inside", false),
        ("list-style:decimal outside", false),
        ("list-style-type:'word '", false),
    ] {
        let item = Element::synthetic(
            "li",
            style(
                &format!("display:list-item;letter-spacing:4px;word-spacing:160px;{css}"),
                vcx,
            ),
            Default::default(),
            vec![],
        );
        let merged = inherit(&BoxStyle::default(), &item.style.used(), vcx);
        let marker = resolve(&item, &merged, &merged, vcx).unwrap();
        let used = marker.style.as_ref().unwrap_or(&merged);
        assert_eq!(
            used.letter_spacing,
            Some(Len::Px(if symbol { 0. } else { 4. })),
            "{css}"
        );
        assert_eq!(
            used.word_spacing,
            Some(Len::Px(if symbol { 0. } else { 160. })),
            "{css}"
        );
    }
}
