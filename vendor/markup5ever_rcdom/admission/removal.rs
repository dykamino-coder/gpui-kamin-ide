//! An in-place child-list edit. Both directions retain the original allocation.
use crate::{Handle, WeakHandle};
use std::rc::Rc;

pub struct Removal {
    parent: Handle,
    link: WeakHandle,
    removed: Vec<(usize, Handle)>,
}

impl Removal {
    /// Capture matching children in sibling order and retain their parent link.
    pub fn prepare(
        parent: &Handle,
        count: usize,
        mut selected: impl FnMut(&Handle) -> bool,
    ) -> Option<Self> {
        let removed: Vec<_> = parent
            .children
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, child)| selected(child))
            .map(|(slot, child)| (slot, child.clone()))
            .collect();
        if removed.len() != count {
            return None;
        }
        let first = &removed.first()?.1;
        let link = first.parent.take()?;
        first.parent.set(Some(link.clone()));
        Some(Self {
            parent: parent.clone(),
            link,
            removed,
        })
    }

    pub fn parent(&self) -> &Handle {
        &self.parent
    }

    pub fn write(&self) {
        let mut remove = self.removed.iter().peekable();
        self.parent.children.borrow_mut().retain(|node| {
            if remove
                .next_if(|(_, child)| Rc::ptr_eq(node, child))
                .is_some()
            {
                node.parent.take();
                false
            } else {
                true
            }
        });
    }

    pub fn undo(&self) {
        // retain preserved capacity. Put the surviving siblings back from right to left.
        let mut children = self.parent.children.borrow_mut();
        let mut survivors = children.len();
        let mut removed = self.removed.iter().rev().peekable();
        children.resize(survivors + self.removed.len(), self.removed[0].1.clone());
        for slot in (0..children.len()).rev() {
            if let Some((_, node)) = removed.next_if(|(at, _)| *at == slot) {
                children[slot] = node.clone();
                node.parent.set(Some(self.link.clone()));
            } else {
                survivors -= 1;
                children.swap(slot, survivors);
            }
        }
    }
}
