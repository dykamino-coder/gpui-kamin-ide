//! Наследование картинки маркера сохраняется при разрыве строчных коробок.

use super::split_block_in_inline;
use crate::document::state::{DocState, ValueCx};
use crate::dom::{Element, Node};
use crate::layout::list::list_item::marker;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::css::parse_decls;

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
            let expected = inherit(&ancestor.style, &item.style, vcx).list_style_image;
            let mut node = Node::Element(item);
            for level in 0..depth {
                let mut parent = ancestor.clone();
                if level == 0 && depth == 2 {
                    parent.style.list_style_image = None;
                }
                parent.children = vec![node];
                node = Node::Element(parent);
            }
            let split = split_block_in_inline(&[node]);
            let [Node::Element(item)] = split.as_slice() else {
                panic!("вынесенный пункт списка")
            };
            let merged = inherit(&Computed::default(), &item.style, vcx);
            assert_eq!(merged.list_style_image, expected, "{own}, depth={depth}");
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
        let merged = inherit(&parent.style, &child.style, vcx);
        assert!(merged.list_style_image.as_ref().unwrap().is_some());
        assert_eq!(
            marker::resolve(&child, &parent.style, &merged, vcx).is_some(),
            display == "list-item"
        );
    }
}
