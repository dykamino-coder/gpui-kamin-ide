//! Stage sibling order for the touched parents. Each original list is indexed once.
//! Insert, remove and move address the same links in journal order.
use super::{Address, ApplyError, parent};
use crate::dom::input::Context;
use markup5ever_rcdom::{Handle, NodeData, WeakHandle, admission::ChildList};
use std::{collections::HashMap, rc::Rc};

#[derive(Default)]
pub(super) struct Requests {
    lists: HashMap<Address, List>,
    links: HashMap<Address, Link>,
}

struct List {
    parent: Handle,
    first: Option<Address>,
    last: Option<Address>,
}

struct Link {
    node: Handle,
    parent: Option<Handle>,
    prev: Option<Address>,
    next: Option<Address>,
}

impl Requests {
    pub(super) fn parents(&self) -> impl Iterator<Item = &Handle> {
        self.lists.values().map(|list| &list.parent)
    }

    fn load(&mut self, parent: &Handle) {
        self.lists.entry(Rc::as_ptr(parent)).or_insert_with(|| {
            let children = parent.children.borrow();
            let id = |n: &Handle| Rc::as_ptr(n);
            for (i, child) in children.iter().enumerate() {
                self.links.insert(
                    id(child),
                    Link {
                        node: child.clone(),
                        parent: Some(parent.clone()),
                        prev: i.checked_sub(1).map(|i| id(&children[i])),
                        next: children.get(i + 1).map(id),
                    },
                );
            }
            List {
                parent: parent.clone(),
                first: children.first().map(id),
                last: children.last().map(id),
            }
        });
    }

    fn current_parent(&self, node: &Handle) -> Option<Handle> {
        self.links
            .get(&Rc::as_ptr(node))
            .map_or_else(|| parent(node), |link| link.parent.clone())
    }

    pub(super) fn remove(&mut self, from: &Handle, child: &Handle) -> Result<(), ApplyError> {
        self.load(from);
        let id = Rc::as_ptr(child);
        let link = self.links.get_mut(&id).ok_or(ApplyError::NotChild)?;
        if !link.parent.as_ref().is_some_and(|p| Rc::ptr_eq(p, from)) {
            return Err(ApplyError::NotChild);
        }
        link.parent = None;
        let (prev, next) = (link.prev.take(), link.next.take());
        let list = self.lists.get_mut(&Rc::as_ptr(from)).unwrap();
        if let Some(prev) = prev {
            self.links.get_mut(&prev).unwrap().next = next;
        } else {
            list.first = next;
        }
        if let Some(next) = next {
            self.links.get_mut(&next).unwrap().prev = prev;
        } else {
            list.last = prev;
        }
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
        self.load(to);
        if let Some(reference) = before {
            if !self
                .current_parent(reference)
                .is_some_and(|p| Rc::ptr_eq(&p, to))
            {
                return Err(ApplyError::NotChild);
            }
            if Rc::ptr_eq(reference, child) {
                return Ok(());
            }
        }
        if let Some(from) = self.current_parent(child) {
            self.remove(&from, child)?;
        }
        let list = self.lists.get_mut(&Rc::as_ptr(to)).unwrap();
        let id = Rc::as_ptr(child);
        let next = before.map(Rc::as_ptr);
        let prev = next.map_or(list.last, |n| self.links[&n].prev);
        if let Some(p) = prev {
            self.links.get_mut(&p).unwrap().next = Some(id);
        } else {
            list.first = Some(id);
        }
        if let Some(n) = next {
            self.links.get_mut(&n).unwrap().prev = Some(id);
        } else {
            list.last = Some(id);
        }
        self.links.insert(
            id,
            Link {
                node: child.clone(),
                parent: Some(to.clone()),
                prev,
                next,
            },
        );
        if matches!(to.data, NodeData::Document) {
            let mut element = false;
            let mut doctype = false;
            let mut next = list.first;
            while let Some(id) = next {
                let link = &self.links[&id];
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
        for list in self.lists.into_values() {
            let ordered = || {
                std::iter::successors(list.first, |id| self.links[id].next)
                    .map(|id| self.links[&id].node.clone())
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
            changes.lists.push((list.parent, children));
        }
        for link in self.links.into_values() {
            let old = parent(&link.node);
            if old.as_ref().map(Rc::as_ptr) != link.parent.as_ref().map(Rc::as_ptr) {
                changes.links.push((
                    link.node,
                    old.as_ref().map(WeakHandle::new),
                    link.parent.as_ref().map(WeakHandle::new),
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

impl Changes {
    pub(super) fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }

    pub(super) fn exchange(&mut self, forward: bool) {
        for (parent, list) in &mut self.lists {
            list.exchange(parent);
        }
        for (node, old, new) in &self.links {
            node.parent.set(if forward { new } else { old }.clone());
        }
    }
}
