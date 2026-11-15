//! Ленивый владелец текстовых результатов для принятого text-only IFC.

use super::{BoxTree, inline::InlineContent};
use crate::{dom::Node, style::computed::Computed, text::paragraph::retained::Retained};
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
pub(super) struct TextResults(RefCell<Option<(Rc<InlineContent>, Rc<Retained>)>>);

impl TextResults {
    pub(super) fn keep(&mut self, text: Option<&Rc<InlineContent>>) {
        if self
            .0
            .get_mut()
            .as_ref()
            .is_some_and(|(old, _)| !text.is_some_and(|text| Rc::ptr_eq(old, text)))
        {
            *self.0.get_mut() = None;
        }
    }
}

impl BoxTree {
    pub(crate) fn text_results(&self, nodes: &[Node], style: &Computed) -> Option<Rc<Retained>> {
        let text = self.normalized(nodes, style)?;
        let id = *self.flows.get(&super::finish::slice_key(nodes))?;
        let record = self.get(id)?;
        let mut retained = record.text_results.0.borrow_mut();
        if let Some((source, owner)) = retained.as_ref()
            && Rc::ptr_eq(source, &text)
        {
            return Some(owner.clone());
        }
        let owner = Rc::new(Retained::default());
        *retained = Some((text, owner.clone()));
        Some(owner)
    }
}

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for TextResults {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        self.0.try_borrow().map_or(0, |entry| {
            entry.as_ref().map_or(0, |(text, owner)| {
                text.heap_size(ops) + owner.heap_size(ops)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    use crate::style::computed::BoxStyle;

    fn owners(doc: &Document) -> Vec<Rc<Retained>> {
        fn walk(nodes: &[Node], tree: &BoxTree, out: &mut Vec<Rc<Retained>>) {
            for node in nodes {
                if let Node::Element(e) = node {
                    if e.tag == "p" {
                        out.push(tree.text_results(&e.children, &e.style.used()).unwrap());
                    }
                    walk(&e.children, tree, out);
                }
            }
        }
        doc.state
            .boxes
            .borrow_mut()
            .finish(&doc.nodes, &BoxStyle::default(), false);
        let mut out = Vec::new();
        walk(&doc.nodes, &doc.state.boxes.borrow(), &mut out);
        out
    }

    #[test]
    fn editing_one_flow_preserves_the_other_and_old_callbacks_keep_their_owner() {
        let mut doc = Document::builder("<p id=a>left</p><p id=b>right</p>", "")
            .build()
            .unwrap();
        let before = owners(&doc);
        doc.update("<p id=a>changed</p><p id=b>right</p>", "")
            .unwrap();
        let after = owners(&doc);
        assert!(!Rc::ptr_eq(&before[0], &after[0]));
        assert!(Rc::ptr_eq(&before[1], &after[1]));
        assert_eq!(
            Rc::strong_count(&before[0]),
            1,
            "only an old callback owns obsolete results"
        );
        doc.update(
            "<p id=a>changed</p><p id=b>right</p>",
            "p { width: 100px; color: red }",
        )
        .unwrap();
        let resized = owners(&doc);
        assert!(Rc::ptr_eq(&after[0], &resized[0]));
        assert!(Rc::ptr_eq(&after[1], &resized[1]));
    }

    #[test]
    fn removed_flows_release_results_and_slot_reuse_does_not_recover_old_owners() {
        let mut tree = BoxTree::default();
        let nodes = [Node::Text("first".into())];
        tree.publish(&nodes);
        let old = tree.text_results(&nodes, &BoxStyle::default()).unwrap();
        let weak = Rc::downgrade(&old);
        tree.publish(&[]);
        assert_eq!(Rc::strong_count(&old), 1);
        let next = [Node::Text("second".into())];
        tree.publish(&next);
        let new = tree.text_results(&next, &BoxStyle::default()).unwrap();
        assert!(!Rc::ptr_eq(&old, &new));
        drop(old);
        assert!(weak.upgrade().is_none());
    }
}
