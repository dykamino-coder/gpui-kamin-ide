//! Наследование картинки маркера сохраняется при разрыве строчных коробок.

use super::split_block_in_inline;
use crate::document::state::DocState;
use crate::dom::{Element, Node};
use crate::layout::list::list_item::marker;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::css::parse_decls;
use crate::values::ValueCx;

fn element(css: &str, children: Vec<Node>, vcx: ValueCx<'_>) -> Element {
    let mut style = Computed::default();
    style.apply_decls_with_vars(&parse_decls(css), &Default::default(), vcx);
    Element::synthetic("span", style, Default::default(), children)
}

#[test]
fn inline_splits_preserve_inherited_marker_images_and_explicit_overrides() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    for own in [
        "",
        "list-style-image:none",
        "list-style-image:linear-gradient(blue, blue)",
    ] {
        for depth in [1, 2] {
            let item = element(&format!("display:list-item; {own}"), vec![], vcx);
            let mut ancestor = element(
                "display:inline; list-style-image:linear-gradient(green, green)",
                vec![],
                vcx,
            );
            ancestor.inline = true;
            let expected = inherit(
                &BoxStyle::shared(ancestor.style.used()),
                &item.style.used(),
                vcx,
            )
            .inh
            .table_list
            .list_style_image
            .clone();
            let mut node = Node::Element(item);
            for level in 0..depth {
                let mut parent = ancestor.clone();
                if level == 0 && depth == 2 {
                    parent.style.used_mut().inh.table_list.list_style_image = None;
                }
                parent.children = vec![node].into();
                node = Node::Element(parent);
            }
            let nodes = [node];
            crate::style::cascade::materialize::materialize(
                &nodes,
                &std::rc::Rc::new(Computed::default()),
                vcx,
            );
            let split = split_block_in_inline(&nodes);
            let [Node::Element(item)] = &split[..] else {
                panic!("вынесенный пункт списка")
            };
            let merged = inherit(&BoxStyle::default(), &item.style.used(), vcx);
            assert_eq!(
                merged.inh.table_list.list_style_image, expected,
                "{own}, depth={depth}"
            );
            let resolved = marker::resolve(item, &Computed::default(), &merged, vcx).unwrap();
            assert_eq!(resolved.image.is_some(), own != "list-style-image:none");
        }
    }
}

#[test]
fn inherited_marker_image_only_generates_a_marker_for_a_list_item() {
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    for display in ["inline", "block", "list-item"] {
        let parent = element(
            "display:inline; list-style-image:linear-gradient(green, green)",
            vec![],
            vcx,
        );
        let child = element(&format!("display:{display}"), vec![], vcx);
        let merged = inherit(
            &BoxStyle::shared(parent.style.used()),
            &child.style.used(),
            vcx,
        );
        assert!(
            merged
                .inh
                .table_list
                .list_style_image
                .as_ref()
                .unwrap()
                .is_some()
        );
        assert_eq!(
            marker::resolve(&child, &parent.style.used(), &merged, vcx).is_some(),
            display == "list-item"
        );
    }
}

#[test]
fn hoisted_list_item_uses_its_element_parent_not_the_inline_layout_copy() {
    use crate::style::cascade::materialize::{box_style, materialize};
    use std::rc::Rc;
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    let item = element("display:list-item", vec![], vcx);
    let mut parent = element(
        "display:inline;list-style-type:square",
        vec![Node::Element(item)],
        vcx,
    );
    parent.inline = true;
    let mut nodes = [Node::Element(parent)];
    materialize(&nodes, &Rc::new(Computed::default()), vcx);
    let Node::Element(copy) = &mut nodes[0] else {
        unreachable!()
    };
    copy.style.used_mut().inh.table_list.list_style_type =
        Some(crate::style::computed::MarkerType::Counter("circle".into()));
    let split = split_block_in_inline(&nodes);
    let [Node::Element(item)] = &split[..] else {
        panic!("list item")
    };
    let boxed = box_style(&BoxStyle::default(), &item.style, vcx);
    assert_eq!(
        boxed.inh.table_list.list_style_type,
        Some(crate::style::computed::MarkerType::Counter("square".into()))
    );
    let marker = marker::resolve(item, &Computed::default(), &boxed, vcx).unwrap();
    assert_eq!(marker.text.trim(), "▪");
}

#[test]
fn steps_without_work_return_the_children_without_a_copy() {
    // Anonymous tables and block-in-inline splits run on every block's
    // children; a list they leave unchanged must not be cloned per step.
    let doc = DocState::new(Default::default());
    let vcx = doc.values();
    let nodes = [
        Node::Element(element("display:block", vec![], vcx)),
        Node::Element(element("display:inline", vec![], vcx)),
    ];
    crate::style::cascade::materialize::materialize(
        &nodes,
        &std::rc::Rc::new(Computed::default()),
        vcx,
    );
    let tables = crate::layout::table::anon::wrap_anon_tables(&nodes);
    assert!(matches!(tables, std::borrow::Cow::Borrowed(_)));
    let split = split_block_in_inline(&tables);
    assert!(matches!(split, std::borrow::Cow::Borrowed(_)));
    assert_eq!(split.len(), 2);
}
