//! Stage sibling order for the touched parents. Each original list is indexed once.
//! Insert, remove and move address the same links in journal order.
use super::{Address, ApplyError, parent};
use crate::dom::input::Context;
use markup5ever_rcdom::{Handle, NodeData, WeakHandle, admission::ChildList};
use std::{collections::HashMap, rc::Rc};

#[derive(Default)]
pub(super) struct Requests {
    list_slots: HashMap<Address, usize>,
    node_slots: HashMap<Address, usize>,
    lists: Vec<List>,
    links: Vec<Link>,
}

struct List {
    parent: Handle,
    first: Option<usize>,
    last: Option<usize>,
}

struct Link {
    node: Handle,
    parent: Option<usize>,
    prev: Option<usize>,
    next: Option<usize>,
}

impl Requests {
    pub(super) fn parents(&self) -> impl Iterator<Item = &Handle> {
        self.lists.iter().map(|list| &list.parent)
    }

    /// A staged node keeps its slot while it moves between sibling lists.
    fn node_slot(&mut self, node: &Handle) -> usize {
        *self.node_slots.entry(Rc::as_ptr(node)).or_insert_with(|| {
            let slot = self.links.len();
            self.links.push(Link {
                node: node.clone(),
                parent: None,
                prev: None,
                next: None,
            });
            slot
        })
    }

    fn load(&mut self, parent: &Handle) -> usize {
        if let Some(&slot) = self.list_slots.get(&Rc::as_ptr(parent)) {
            return slot;
        }
        let slot = self.lists.len();
        let mut list = List {
            parent: parent.clone(),
            first: None,
            last: None,
        };
        for child in parent.children.borrow().iter() {
            let id = self.node_slot(child);
            self.links[id].parent = Some(slot);
            self.links[id].prev = list.last;
            if let Some(previous) = list.last {
                self.links[previous].next = Some(id);
            } else {
                list.first = Some(id);
            }
            list.last = Some(id);
        }
        self.lists.push(list);
        self.list_slots.insert(Rc::as_ptr(parent), slot);
        slot
    }

    fn current_parent(&self, node: &Handle) -> Option<Handle> {
        match self.node_slots.get(&Rc::as_ptr(node)) {
            Some(&id) => self.links[id]
                .parent
                .map(|slot| self.lists[slot].parent.clone()),
            None => parent(node),
        }
    }

    /// Join the neighbors across a removal, or place one slot between them.
    fn splice(
        &mut self,
        list: usize,
        prev: Option<usize>,
        next: Option<usize>,
        middle: Option<usize>,
    ) {
        if let Some(prev) = prev {
            self.links[prev].next = middle.or(next);
        } else {
            self.lists[list].first = middle.or(next);
        }
        if let Some(next) = next {
            self.links[next].prev = middle.or(prev);
        } else {
            self.lists[list].last = middle.or(prev);
        }
    }

    pub(super) fn remove(&mut self, from: &Handle, child: &Handle) -> Result<(), ApplyError> {
        let list = self.load(from);
        let id = *self
            .node_slots
            .get(&Rc::as_ptr(child))
            .ok_or(ApplyError::NotChild)?;
        let link = &mut self.links[id];
        if link.parent != Some(list) {
            return Err(ApplyError::NotChild);
        }
        link.parent = None;
        let (prev, next) = (link.prev.take(), link.next.take());
        self.splice(list, prev, next, None);
        Ok(())
    }

    pub(super) fn insert(
        &mut self,
        to: &Handle,
        child: &Handle,
        before: Option<&Handle>,
    ) -> Result<(), ApplyError> {
        if !matches!(to.data, NodeData::Element { .. } | NodeData::Document)
            || matches!(child.data, NodeData::Document)
            || matches!(child.data, NodeData::Doctype { .. })
                && !matches!(to.data, NodeData::Document)
        {
            return Err(ApplyError::Hierarchy);
        }
        let mut ancestor = Some(to.clone());
        while let Some(node) = ancestor {
            if Rc::ptr_eq(&node, child) {
                return Err(ApplyError::Hierarchy);
            }
            ancestor = self.current_parent(&node);
        }
        let list = self.load(to);
        let next = if let Some(reference) = before {
            let slot = self
                .node_slots
                .get(&Rc::as_ptr(reference))
                .copied()
                .filter(|slot| self.links[*slot].parent == Some(list))
                .ok_or(ApplyError::NotChild)?;
            if Rc::ptr_eq(reference, child) {
                return Ok(());
            }
            Some(slot)
        } else {
            None
        };
        if let Some(from) = self.current_parent(child) {
            self.remove(&from, child)?;
        }
        let id = self.node_slot(child);
        let prev = next.map_or(self.lists[list].last, |slot| self.links[slot].prev);
        let link = &mut self.links[id];
        link.parent = Some(list);
        link.prev = prev;
        link.next = next;
        self.splice(list, prev, next, Some(id));
        if matches!(to.data, NodeData::Document) {
            let mut element = false;
            let mut doctype = false;
            let mut next = self.lists[list].first;
            while let Some(id) = next {
                let link = &self.links[id];
                match link.node.data {
                    NodeData::Text { .. } => return Err(ApplyError::Hierarchy),
                    NodeData::Element { .. } if std::mem::replace(&mut element, true) => {
                        return Err(ApplyError::Hierarchy);
                    }
                    NodeData::Doctype { .. }
                        if element || std::mem::replace(&mut doctype, true) =>
                    {
                        return Err(ApplyError::Hierarchy);
                    }
                    _ => {}
                }
                next = link.next;
            }
        }
        Ok(())
    }

    pub(super) fn finish(self, context: impl FnOnce() -> Context) -> Result<Changes, ApplyError> {
        let mut changes = Changes::default();
        if self.lists.is_empty() {
            return Ok(changes);
        }
        let context = context();
        for list in &self.lists {
            let ordered = || {
                std::iter::successors(list.first, |id| self.links[*id].next)
                    .map(|id| self.links[id].node.clone())
            };
            if ordered().map(|n| Rc::as_ptr(&n)).eq(list
                .parent
                .children
                .borrow()
                .iter()
                .map(Rc::as_ptr))
            {
                continue;
            }
            let children = context
                .child_list(ordered())
                .map_err(ApplyError::Rejected)?;
            changes.lists.push((list.parent.clone(), children));
        }
        for link in self.links {
            let new = link.parent.map(|slot| &self.lists[slot].parent);
            let old = parent(&link.node);
            if old.as_ref().map(Rc::as_ptr) != new.map(Rc::as_ptr) {
                changes.links.push((
                    link.node,
                    old.as_ref().map(WeakHandle::new),
                    new.map(WeakHandle::new),
                ));
            }
        }
        Ok(changes)
    }
}

#[derive(Default)]
pub(super) struct Changes {
    lists: Vec<(Handle, ChildList)>,
    links: Vec<(Handle, Option<WeakHandle>, Option<WeakHandle>)>,
}

mod delta;

impl Changes {
    pub(super) fn exchange(&mut self, forward: bool) {
        for (parent, list) in &mut self.lists {
            list.exchange(parent);
        }
        for (node, old, new) in &self.links {
            node.parent.set(if forward { new } else { old }.clone());
        }
    }
}
