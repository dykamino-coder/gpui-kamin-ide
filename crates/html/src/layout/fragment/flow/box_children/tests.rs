//! Генерация коробок совпадает у eligibility, рекордера и корневых копий.
use super::*;
use crate::dom::{Element, Node};
use crate::layout::fragment::{
    ShapeCx,
    flow::{blocks::BlockBox, independent_copy, vertical_copy},
};
use crate::style::computed::Computed;
use crate::style::values::value::Len;

fn root(nodes: &[Node]) -> &Element {
    fn find(nodes: &[Node]) -> Option<&Element> {
        nodes.iter().find_map(|n| match n {
            Node::Element(e) if e.tag == "div" => Some(e),
            Node::Element(e) => find(&e.children),
            _ => None,
        })
    }
    find(nodes).expect("authored root")
}

#[test]
fn transparent_and_hidden_nodes_do_not_become_recorded_boxes() {
    for mode in ["horizontal-tb", "vertical-lr", "vertical-rl"] {
        for body in [
            "<div>text</div>",
            "<div style='display:grid'><div></div></div>",
            "<div style='display:flex'><div></div></div>",
            "<table><tr><td><div></div></td></tr></table>",
        ] {
            let tree = crate::dom::parse(
                &format!(
                    "<div style='writing-mode:{mode}'><div style='display:contents;box-decoration-break:clone;columns:2'><div style='display:none;writing-mode:horizontal-tb;box-decoration-break:clone'><div style='position:absolute'></div></div>{body}</div></div>"
                ),
                "",
            );
            let mut e = root(&tree).clone();
            // Парсер удаляет display:none; позднее изменение стиля должно
            // давать тот же набор коробок без повторного разбора DOM.
            let mut hidden = e.clone();
            hidden.node_id = u64::MAX;
            hidden.style.display = Some(crate::style::computed::Display::None);
            let hidden_id = hidden.node_id;
            let Node::Element(contents) = &mut e.children[0] else {
                panic!("contents")
            };
            contents.children.insert(0, Node::Element(hidden));
            let Node::Element(contents) = &e.children[0] else {
                panic!("contents")
            };
            let logical = if mode == "horizontal-tb" {
                assert!(crate::layout::fragment::push::measured_tree(&e), "{body}");
                // ADR P-19 TRANSITIONAL(F2-4): рекордер видит flex, но независимой копии
                // ещё нужны отдельные продолжения его параллельных потоков.
                assert_eq!(independent_copy(&e), !body.contains("display:flex"));
                e.clone()
            } else {
                assert!(vertical_copy(&e, mode == "vertical-rl"), "{mode}: {body}");
                crate::layout::fragment::flow::logical_copy(&e, mode == "vertical-rl").unwrap()
            };
            let b = BlockBox::collect(&logical, ShapeCx::COLUMNS, tree.values());
            let ids = b.ids();
            assert!(!ids.contains(&contents.node_id));
            assert!(!ids.contains(&hidden_id));
            for n in children(&contents.children) {
                if let Node::Element(k) = n {
                    assert!(ids.contains(&k.node_id), "{mode}: {body}");
                }
            }
        }
    }
}

#[test]
fn promoted_copies_keep_inheritance_without_contents_box_properties() {
    let tree = crate::dom::parse(
        "<div><div style='display:contents;font-size:20px;color:green;padding:99px'><div style='display:contents;font-size:150%'>text<div style='font-size:200%;color:inherit'>child</div></div></div></div>",
        "",
    );
    let e = root(&tree);
    let nodes = projected(&e.children, &Computed::default(), tree.values());
    assert_eq!(nodes.len(), 2);
    let [Node::Element(text), Node::Element(block)] = nodes.as_slice() else {
        panic!("copies")
    };
    assert!(text.inline);
    assert_eq!(text.style.font_size, Some(Len::Px(30.0)));
    assert_eq!(block.style.font_size, Some(Len::Px(60.0)));
    assert_eq!(text.style.color, block.style.color);
    assert_eq!(text.style.padding.top, None);
    assert_eq!(block.style.padding.top, None);
    let resolved = crate::style::cascade::inherit::inherit_unpainted(
        &Computed::default(),
        &block.style,
        tree.values(),
    );
    assert_eq!(resolved.font_size, block.style.font_size);
    assert_eq!(resolved.color, block.style.color);
}

#[test]
fn promoted_explicit_inheritance_is_not_rebound_to_box_parent() {
    use crate::style::cascade::inherit::inherit_unpainted;
    let tree = crate::dom::parse(
        "<div style='padding:3px;margin:4px;width:5px;font-size:16px'><div style='display:contents;padding:11px;margin:12px;width:13px;background:green;font-size:24px'><div style='display:contents'><div style='padding:inherit;margin:inherit;width:inherit;background:inherit;font-size:larger'>child</div></div></div></div>",
        "",
    );
    let root = root(&tree);
    let parent = inherit_unpainted(&Computed::default(), &root.style, tree.values());
    let projected = projected(&root.children, &parent, tree.values());
    let [Node::Element(copy)] = projected.as_slice() else {
        panic!("one copy")
    };
    let rebound = inherit_unpainted(&parent, &copy.style, tree.values());
    // Вложенный contents наследует только явно запрошенные box-свойства:
    // без собственных padding/margin/width он передаёт начальные значения.
    assert_eq!(rebound.padding.top, None);
    assert_eq!(rebound.margin.top, None);
    assert_eq!(rebound.width, None);
    assert_eq!(rebound.background, None);
    assert_eq!(rebound.font_size, Some(Len::Px(32.0)));

    let mut contents = root.children.clone();
    let Node::Element(outer) = &mut contents[0] else {
        panic!("outer")
    };
    let Node::Element(inner) = &mut outer.children[0] else {
        panic!("inner")
    };
    inner.style.padding_inherit = true;
    inner.style.margin_inherit = [true; 4];
    inner.style.width_inherit = true;
    inner.style.background_inherit = true;
    let projected = super::projected(&contents, &parent, tree.values());
    let [Node::Element(copy)] = projected.as_slice() else {
        panic!("one copy")
    };
    let rebound = inherit_unpainted(&parent, &copy.style, tree.values());
    assert_eq!(rebound.padding.top, Some(Len::Px(11.0)));
    assert_eq!(rebound.margin.top, Some(Len::Px(12.0)));
    assert_eq!(rebound.width, Some(Len::Px(13.0)));
    assert!(rebound.background.is_some());
    assert_eq!(rebound.font_size, Some(Len::Px(32.0)));
}
