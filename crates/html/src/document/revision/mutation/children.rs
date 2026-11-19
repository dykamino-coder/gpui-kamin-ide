//! Removal plans for the child lists touched by one batch.
use super::{ApplyError, parent};
use markup5ever_rcdom::{Handle, Node, WeakHandle};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Default)]
pub(super) struct Requests<'a> {
    parents: HashMap<*const Node, (&'a Handle, usize)>,
    detached: HashSet<*const Node>,
}

impl<'a> Requests<'a> {
    pub(super) fn remove(&mut self, from: &'a Handle, child: &Handle) -> Result<(), ApplyError> {
        if parent(child).is_none_or(|actual| !Rc::ptr_eq(&actual, from))
            || !self.detached.insert(Rc::as_ptr(child))
        {
            return Err(ApplyError::NotChild);
        }
        let (_, count) = self.parents.entry(Rc::as_ptr(from)).or_insert((from, 0));
        *count += 1;
        Ok(())
    }

    pub(super) fn finish(self) -> Result<Vec<Removal>, ApplyError> {
        let mut lists = Vec::with_capacity(self.parents.len());
        for (parent, count) in self.parents.into_values() {
            let removed: Vec<_> = parent
                .children
                .borrow()
                .iter()
                .enumerate()
                .filter(|(_, child)| self.detached.contains(&Rc::as_ptr(child)))
                .map(|(slot, child)| (slot, child.clone()))
                .collect();
            if removed.len() != count {
                return Err(ApplyError::NotChild);
            }
            // Preserve the parent's allocation lease along with its weak link.
            let first = &removed[0].1;
            let link = first.parent.take().ok_or(ApplyError::NotChild)?;
            first.parent.set(Some(link.clone()));
            lists.push(Removal {
                parent: parent.clone(),
                link,
                removed,
            });
        }
        Ok(lists)
    }
}

pub(super) struct Removal {
    pub(super) parent: Handle,
    link: WeakHandle,
    removed: Vec<(usize, Handle)>,
}

impl Removal {
    pub(super) fn write(&self) {
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

    pub(super) fn undo(&self) {
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
