//! Suffix answers equal the walk over all later siblings, and follow DOM
//! insertion, removal and style changes of a sibling or its descendant.

use super::Siblings;
use crate::dom::{Children, Element, Node};
use crate::layout::positioned::predicates::{edge_set, later_node, stays_node};
use crate::style::computed::{Computed, Position};

fn style(css: &str) -> Computed {
    let doc = crate::document::state::DocState::new(Default::default());
    let mut style = Computed::default();
    style.apply_decls(&crate::style::css::parse_decls(css), doc.values());
    style
}

fn el(css: &str, children: Vec<Node>) -> Node {
    Node::Element(Element::synthetic(
        "div",
        style(css),
        Default::default(),
        children,
    ))
}

const STATIC: &str = "";
const HOISTED: &str = "position:absolute;left:0;top:0";
const IN_PLACE_ABS: &str = "position:absolute";
const RELATIVE: &str = "position:relative";
const BELOW: &str = "position:relative;z-index:-1";
const CB: &str = "transform:scale(1)";

/// The walk before the suffix answers: every question visits all later
/// siblings and their subtrees.
fn reference_stays(nodes: &[Node], under_cb: bool) -> bool {
    nodes.iter().any(|n| {
        let Node::Element(e) = n else { return false };
        let s = e.style.used();
        let pos = s.reset_positioned.position;
        let positioned = matches!(
            pos,
            Some(Position::Relative) | Some(Position::Sticky) | Some(Position::Absolute)
        );
        let i = &s.reset_positioned.inset;
        let hoisted = pos == Some(Position::Absolute)
            && !under_cb
            && s.reset_positioned.z_index.unwrap_or(0) >= 0
            && (edge_set(i.left.clone())
                || edge_set(i.right.clone())
                || edge_set(i.top.clone())
                || edge_set(i.bottom.clone()));
        if positioned && s.reset_positioned.z_index.is_some_and(|z| z < 0) {
            return false;
        }
        if positioned && !hoisted {
            return true;
        }
        if hoisted {
            return false;
        }
        reference_stays(
            &e.children,
            under_cb || crate::text::inline::establishes_cb(&s),
        )
    })
}

fn lists() -> Vec<Vec<Node>> {
    vec![
        vec![],
        vec![Node::Text("t".into())],
        vec![
            el(HOISTED, vec![]),
            el(HOISTED, vec![]),
            el(HOISTED, vec![]),
        ],
        vec![
            el(HOISTED, vec![]),
            el(RELATIVE, vec![]),
            el(HOISTED, vec![]),
        ],
        vec![
            el(HOISTED, vec![]),
            el(STATIC, vec![el(STATIC, vec![el(IN_PLACE_ABS, vec![])])]),
            Node::Text("t".into()),
            el(HOISTED, vec![el(RELATIVE, vec![])]),
        ],
        vec![
            el(BELOW, vec![el(RELATIVE, vec![])]),
            el(STATIC, vec![]),
            el(HOISTED, vec![]),
        ],
        // Under a containing block an absolute child stays in place.
        vec![
            el(HOISTED, vec![]),
            el(CB, vec![el(HOISTED, vec![])]),
            el(STATIC, vec![]),
        ],
        vec![el(
            STATIC,
            vec![el(CB, vec![el(STATIC, vec![el(HOISTED, vec![])])])],
        )],
    ]
}

#[test]
fn suffix_answers_equal_the_walk_over_all_later_siblings() {
    for nodes in lists() {
        let siblings = Siblings::new(&nodes);
        for idx in 0..nodes.len() {
            let rest = &nodes[idx + 1..];
            assert_eq!(
                siblings.after(idx).stays_positioned(),
                reference_stays(rest, false)
            );
            assert_eq!(
                siblings.after(idx).stays_positioned(),
                rest.iter()
                    .any(|n| matches!(n, Node::Element(e) if stays_node(e, false)))
            );
            assert_eq!(
                siblings.after(idx).positioned_later(),
                rest.iter()
                    .any(|n| matches!(n, Node::Element(e) if later_node(e)))
            );
        }
        // Asked again, the snapshot answers agree with the first walk.
        for idx in 0..nodes.len() {
            assert_eq!(
                siblings.after(idx).stays_positioned(),
                reference_stays(&nodes[idx + 1..], false)
            );
        }
    }
}

fn first_stays(list: &Children) -> bool {
    Siblings::new(list).after(0).stays_positioned()
}

fn first_later(list: &Children) -> bool {
    Siblings::new(list).after(0).positioned_later()
}

fn child(list: &mut Children, i: usize) -> &mut Element {
    match &mut list[i] {
        Node::Element(e) => e,
        Node::Text(_) => unreachable!(),
    }
}

#[test]
fn answers_follow_insertion_removal_and_style_changes_of_siblings_and_descendants() {
    // hoisted, then a static sibling with a static grandchild.
    let mut list: Children = vec![el(HOISTED, vec![]), el(STATIC, vec![el(STATIC, vec![])])].into();
    assert!(!first_stays(&list));
    assert!(!first_later(&list));
    // A descendant of a later sibling becomes positioned: the cached subtree
    // answer of that sibling must be forgotten.
    child(&mut child(&mut list, 1).children, 0).style = style(RELATIVE).into();
    assert!(first_stays(&list));
    assert!(first_later(&list));
    // …and static again.
    child(&mut child(&mut list, 1).children, 0).style = style(STATIC).into();
    assert!(!first_stays(&list));
    assert!(!first_later(&list));
    // Insertion of a positioned sibling.
    list.push(el(RELATIVE, vec![]));
    assert!(first_stays(&list));
    // Its style change: a hoisted sibling moves to the layer with the first.
    child(&mut list, 2).style = style(HOISTED).into();
    assert!(!first_stays(&list));
    assert!(first_later(&list));
    // Removal.
    list.pop();
    assert!(!first_later(&list));
    // A shared copy keeps its own answers after the original changes.
    let copy = list.clone();
    list.insert(1, el(IN_PLACE_ABS, vec![]));
    assert!(first_stays(&list));
    assert!(!first_stays(&copy));
}
