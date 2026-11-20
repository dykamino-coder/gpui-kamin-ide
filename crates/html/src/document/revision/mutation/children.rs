//! Removal plans for the child lists touched by one batch.
use super::{ApplyError, parent};
pub(super) use markup5ever_rcdom::admission::Removal;
use markup5ever_rcdom::{Handle, Node};
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
            let removal = Removal::prepare(parent, count, |child| {
                self.detached.contains(&Rc::as_ptr(child))
            })
            .ok_or(ApplyError::NotChild)?;
            lists.push(removal);
        }
        Ok(lists)
    }
}
