//! Постоянные intrinsic-размеры узла: неизменяемые входы и локальный dirty-слот.
//! Список детей отделяет слот при COW, материализация помечает цепочку предков.

use crate::style::computed::Computed;
use std::cell::Cell;
use std::rc::Rc;

pub type IntrinsicSizes = Rc<Cell<Option<(f32, f32)>>>;

/// Входы измерения без доступной ширины: она относится к placement, а не min/max.
#[derive(Clone, Debug, PartialEq)]
pub struct IntrinsicInput {
    pub styles: Vec<Rc<Computed>>,
    pub viewport: (f32, f32),
    pub containing: (Option<f32>, Option<f32>),
    pub em: f32,
    pub fonts: u64,
    pub images: u64,
    pub vertical: bool,
}

#[derive(Default)]
pub(super) struct IntrinsicCache(Cell<Option<Rc<Entry>>>);

#[derive(Debug)]
struct Entry {
    input: IntrinsicInput,
    sizes: IntrinsicSizes,
}

impl std::fmt::Debug for IntrinsicCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("IntrinsicCache")
            .field(&self.entry())
            .finish()
    }
}

impl IntrinsicCache {
    fn entry(&self) -> Option<Rc<Entry>> {
        // Запись неизменяема: Cell передаёт владение указателем, а не выдаёт
        // изменяемый доступ к данным узла. До возврата слот восстановлен.
        let entry = self.0.take();
        self.0.set(entry.clone());
        entry
    }

    pub(super) fn dirty(&self) {
        if let Some(entry) = self.0.take() {
            entry.sizes.set(None);
        }
    }

    pub(super) fn sizes(&self, input: IntrinsicInput) -> IntrinsicSizes {
        if let Some(entry) = self.entry()
            && entry.input == input
        {
            return entry.sizes.clone();
        }
        let sizes = IntrinsicSizes::default();
        self.0.set(Some(Rc::new(Entry {
            input,
            sizes: sizes.clone(),
        })));
        sizes
    }
}

#[cfg(test)]
mod tests {
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
                    float: Some(1),
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
        first.style.cascaded_mut().float = Some(1);
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
}
