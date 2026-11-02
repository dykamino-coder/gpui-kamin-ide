//! Тесты постоянного intrinsic-слота узла.

use super::*;
use crate::dom::{Children, Element, Node};

fn input() -> IntrinsicInput {
    IntrinsicInput {
        styles: vec![Rc::default()],
        viewport: (800., 600.),
        containing: (None, None),
        em: 16.,
        fonts: 0,
        images: 0,
        vertical: false,
    }
}

#[test]
fn immutable_node_reuses_pair_and_style_or_resource_changes_miss() {
    let children = Children::default();
    let slot = children.intrinsic_sizes(input());
    slot.set(Some((20., 60.)));
    assert!(Rc::ptr_eq(&slot, &children.intrinsic_sizes(input())));
    for changed in [
        IntrinsicInput {
            fonts: 1,
            ..input()
        },
        IntrinsicInput {
            images: 1,
            ..input()
        },
        IntrinsicInput {
            viewport: (900., 600.),
            ..input()
        },
        IntrinsicInput {
            styles: vec![Rc::new(Computed {
                reset_box_layout: crate::style::computed::StyleGroup::new(
                    crate::style::computed::ResetBoxLayout {
                        float: Some(1),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            })],
            ..input()
        },
    ] {
        assert!(children.intrinsic_sizes(changed).get().is_none());
    }
}

#[test]
fn inspecting_the_cache_preserves_its_owned_entry_and_dirty_handle() {
    let cache = IntrinsicCache::default();
    let sizes = cache.sizes(input());
    sizes.set(Some((20., 60.)));
    assert!(format!("{cache:?}").contains("IntrinsicCache"));
    assert!(Rc::ptr_eq(&sizes, &cache.sizes(input())));
    cache.dirty();
    assert_eq!(sizes.get(), None);
    assert!(!Rc::ptr_eq(&sizes, &cache.sizes(input())));
}

#[test]
fn weak_identity_mark_keeps_neither_intrinsic_entry_nor_nodes_alive() {
    let children: Children = vec![Node::Text("leaf".into())].into();
    let sizes = children.intrinsic_sizes(input());
    let weak_sizes = Rc::downgrade(&sizes);
    let mark = children.mark();
    drop(sizes);
    assert!(mark.is_alive());
    assert!(weak_sizes.upgrade().is_some());
    drop(children);
    assert!(!mark.is_alive());
    assert!(weak_sizes.upgrade().is_none());
}

#[test]
fn text_and_list_mutations_detach_the_dirty_branch() {
    let mut children: Children = vec![Node::Text("old".into())].into();
    let copy = children.clone();
    children.intrinsic_sizes(input()).set(Some((20., 60.)));
    let Node::Text(text) = &mut children[0] else {
        unreachable!()
    };
    text.push_str(" changed");
    assert!(children.intrinsic_sizes(input()).get().is_none());
    assert_eq!(copy.intrinsic_sizes(input()).get(), Some((20., 60.)));
    children.intrinsic_sizes(input()).set(Some((30., 80.)));
    children.push(Node::Text("new".into()));
    assert!(children.intrinsic_sizes(input()).get().is_none());
}

#[test]
fn unique_snapshot_invalidates_a_previously_borrowed_pair() {
    let mut children: Children = vec![Node::Text("old".into())].into();
    let slot = children.intrinsic_sizes(input());
    slot.set(Some((20., 60.)));
    children.push(Node::Text("new".into()));
    assert!(slot.get().is_none());
    let replacement = children.intrinsic_sizes(input());
    assert!(!Rc::ptr_eq(&slot, &replacement));
    replacement.set(Some((30., 80.)));
    children.dirty_intrinsic_sizes();
    assert!(replacement.get().is_none());
}

#[test]
fn descendant_style_mutation_dirties_ancestors_but_keeps_the_sibling() {
    let leaf = || {
        Node::Element(Element::synthetic(
            "span",
            Default::default(),
            Default::default(),
            vec![],
        ))
    };
    let mut children: Children = vec![leaf(), leaf()].into();
    children.intrinsic_sizes(input()).set(Some((20., 60.)));
    for node in children.iter() {
        let Node::Element(e) = node else {
            unreachable!()
        };
        e.children.intrinsic_sizes(input()).set(Some((10., 30.)));
    }
    let Node::Element(first) = &mut children[0] else {
        unreachable!()
    };
    first.style.cascaded_mut().reset_box_layout.float = Some(1);
    assert!(children.intrinsic_sizes(input()).get().is_none());
    let Node::Element(sibling) = &children[1] else {
        unreachable!()
    };
    assert_eq!(
        sibling.children.intrinsic_sizes(input()).get(),
        Some((10., 30.))
    );
}

#[test]
fn computed_publication_dirties_only_its_ancestor_chain() {
    let parsed = crate::dom::parse("<div><p>a</p><aside>b</aside></div>", "");
    let parent = Rc::default();
    let materialize = || {
        crate::style::cascade::materialize::materialize_changed(
            &parsed.nodes,
            &parent,
            parsed.values(),
        )
    };
    materialize();
    let mut elements = vec![];
    let mut pending = vec![parsed.nodes.as_slice()];
    while let Some(nodes) = pending.pop() {
        for node in nodes {
            if let Node::Element(e) = node {
                elements.push(e);
                pending.push(&e.children);
            }
        }
    }
    for e in &elements {
        e.children.intrinsic_sizes(input()).set(Some((20., 60.)));
    }
    assert!(!materialize());
    let target = elements.iter().find(|e| e.tag == "p").unwrap();
    target.style.slot().set_container_size((Some(123.), None));
    assert!(materialize());
    for e in elements {
        let actual = e.children.intrinsic_sizes(input()).get();
        if matches!(e.tag.as_str(), "p" | "div" | "body" | "html") {
            assert!(actual.is_none(), "{} must become dirty", e.tag);
        } else {
            assert_eq!(actual, Some((20., 60.)), "unrelated sibling stays clean");
        }
    }
}

#[test]
fn equal_value_snapshot_hits_and_scalar_change_misses_before_style_compare() {
    let children = Children::default();
    let slot = children.intrinsic_sizes(input());
    slot.set(Some((5., 9.)));
    // Another snapshot with the same computed values is the same input.
    let same = IntrinsicInput {
        styles: vec![Rc::new(Computed::default())],
        ..input()
    };
    assert!(Rc::ptr_eq(&slot, &children.intrinsic_sizes(same)));
    // A one-pixel relayout width change is rejected by the scalars.
    let moved = IntrinsicInput {
        containing: (Some(799.), None),
        ..input()
    };
    assert_ne!(moved, input());
    assert!(children.intrinsic_sizes(moved).get().is_none());
}
