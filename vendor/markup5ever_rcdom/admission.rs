//! Storage reservations precede allocation and travel with their physical owner.

use crate::{Handle, Node, NodeData, RcDom};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::TryReserveError,
    convert::Infallible,
    mem,
    rc::Rc,
    sync::Arc,
};
use tendril::StrTendril;

mod mutations;
mod selectedcontent;

/// None is used by the original, infallible API only.
pub type Lease = Option<Arc<dyn Any + Send + Sync>>;

/// A bounded sink supplies its document policy, including logical DOM maxima.
pub trait Admission {
    type Error;
    fn storage(&self, bytes: usize) -> Result<Lease, Self::Error>;
    fn node(&self, bytes: usize) -> Result<Lease, Self::Error>;
    fn node_count(&self) -> Result<Lease, Self::Error> {
        Ok(None)
    }
    fn payload(&self, text: usize, attributes: usize) -> Result<Lease, Self::Error>;
    /// A replacement checks the resulting logical size, rather than charging
    /// the old and new payload twice. Physical buffers retain separate leases.
    fn replace_payload(
        &self,
        text: usize,
        attributes: usize,
        previous: &Lease,
    ) -> Result<Lease, Self::Error> {
        let _ = previous;
        self.payload(text, attributes)
    }
    fn buffer(&self, text: &mut StrTendril, additional: usize) -> Result<(), Self::Error>;
    fn visit(&self) -> Result<(), Self::Error>;
    fn allocation(&self, error: TryReserveError) -> Self::Error;
    fn overflow(&self) -> Self::Error;
}

pub(crate) struct Unbounded;
impl Admission for Unbounded {
    type Error = Infallible;
    fn storage(&self, _: usize) -> Result<Lease, Infallible> {
        Ok(None)
    }
    fn node(&self, _: usize) -> Result<Lease, Infallible> {
        Ok(None)
    }
    fn payload(&self, _: usize, _: usize) -> Result<Lease, Infallible> {
        Ok(None)
    }
    fn buffer(&self, _: &mut StrTendril, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    fn visit(&self) -> Result<(), Infallible> {
        Ok(())
    }
    fn allocation(&self, _: TryReserveError) -> Infallible {
        panic!("allocation failed")
    }
    fn overflow(&self) -> Infallible {
        panic!("capacity overflow")
    }
}

pub(crate) fn infallible<T>(result: Result<T, Infallible>) -> T {
    result.unwrap_or_else(|never| match never {})
}

/// Replacing storage keeps both generations charged until the old Vec is freed.
pub(crate) fn reserve<T, A: Admission>(
    values: &mut Vec<T>,
    lease: &mut Lease,
    additional: usize,
    policy: &A,
) -> Result<(), A::Error> {
    if additional <= values.capacity() - values.len() {
        return Ok(());
    }
    let required = values
        .len()
        .checked_add(additional)
        .ok_or_else(|| policy.overflow())?;
    let capacity = required
        .max(4)
        .checked_next_power_of_two()
        .ok_or_else(|| policy.overflow())?;
    let bytes = capacity
        .checked_mul(mem::size_of::<T>())
        .ok_or_else(|| policy.overflow())?;
    let next_lease = policy.storage(bytes)?;
    let mut next = Vec::new();
    next.try_reserve_exact(capacity)
        .map_err(|error| policy.allocation(error))?;
    next.append(values);
    let previous = mem::replace(values, next);
    drop(previous);
    *lease = next_lease;
    Ok(())
}

pub(crate) struct Work<T> {
    pub(crate) values: Vec<T>,
    lease: Lease,
}
impl<T> Work<T> {
    pub(crate) fn new() -> Self {
        Self {
            values: Vec::new(),
            lease: None,
        }
    }
    pub(crate) fn push<A: Admission>(&mut self, value: T, policy: &A) -> Result<(), A::Error> {
        reserve(&mut self.values, &mut self.lease, 1, policy)?;
        self.values.push(value);
        Ok(())
    }
    pub(crate) fn pop(&mut self) -> Option<T> {
        self.values.pop()
    }
}

pub(crate) fn payload<A: Admission>(
    data: &NodeData,
    policy: &A,
) -> Result<(usize, usize), A::Error> {
    let sum = |parts: &[usize]| {
        parts.iter().try_fold(0usize, |total, &part| {
            total.checked_add(part).ok_or_else(|| policy.overflow())
        })
    };
    Ok(match data {
        NodeData::Text { contents } => (contents.borrow().len(), 0),
        NodeData::Comment { contents } => (contents.len(), 0),
        NodeData::ProcessingInstruction { target, contents } => {
            (sum(&[target.len(), contents.len()])?, 0)
        }
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => (sum(&[name.len(), public_id.len(), system_id.len()])?, 0),
        NodeData::Element { attrs, .. } => (
            0,
            attrs.borrow().iter().try_fold(0usize, |total, attr| {
                sum(&[
                    total,
                    attr.name.local.len(),
                    attr.name.ns.len(),
                    attr.name.prefix.as_ref().map_or(0, |prefix| prefix.len()),
                    attr.value.len(),
                ])
            })?,
        ),
        NodeData::Document => (0, 0),
    })
}

impl Node {
    pub(crate) fn try_new<A: Admission>(
        data: NodeData,
        attrs_lease: Lease,
        policy: &A,
    ) -> Result<Handle, A::Error> {
        let logical_node = policy.node_count()?;
        let node_lease = policy.node(mem::size_of::<Node>() + 2 * mem::size_of::<usize>())?;
        let (text, attributes) = payload(&data, policy)?;
        let payload_lease = policy.payload(text, attributes)?;
        Ok(Rc::new(Node {
            parent: Cell::new(None),
            children: RefCell::new(Vec::new()),
            data,
            drop_next: RefCell::new(None),
            children_lease: RefCell::new(None),
            _logical_node: logical_node,
            attrs_lease: RefCell::new(attrs_lease),
            payload_lease: RefCell::new(payload_lease),
            _node_lease: node_lease,
        }))
    }
    pub(crate) fn reserve_children<A: Admission>(
        &self,
        additional: usize,
        policy: &A,
    ) -> Result<(), A::Error> {
        reserve(
            &mut self.children.borrow_mut(),
            &mut self.children_lease.borrow_mut(),
            additional,
            policy,
        )
    }
}

impl RcDom {
    /// The document node is admitted before Rc allocation.
    pub fn try_new<A: Admission>(policy: &A) -> Result<Self, A::Error> {
        Ok(Self {
            document: Node::try_new(NodeData::Document, None, policy)?,
            errors: RefCell::new(Vec::new()),
            quirks_mode: Cell::new(markup5ever::interface::tree_builder::NoQuirks),
            errors_lease: RefCell::new(None),
        })
    }
}

#[cfg(test)]
mod tests;

/// The parent is restored before a fallible scan; rejection leaves no dangling
/// or missing parent pointer, even while the caller unwinds partial state.
pub(crate) fn parent_index<A: Admission>(
    target: &Handle,
    policy: &A,
) -> Result<Option<(Handle, usize)>, A::Error> {
    let Some(weak) = target.parent() else {
        return Ok(None);
    };
    let parent = weak.upgrade().expect("dangling weak pointer");
    let mut found = None;
    for (index, child) in parent.children.borrow().iter().enumerate() {
        policy.visit()?;
        if Rc::ptr_eq(child, target) {
            found = Some(index);
            break;
        }
    }
    Ok(Some((
        parent,
        found.expect("a parent must contain its child"),
    )))
}

pub(crate) fn remove<A: Admission>(target: &Handle, policy: &A) -> Result<(), A::Error> {
    if let Some((parent, index)) = parent_index(target, policy)? {
        parent.children.borrow_mut().remove(index);
        target.parent.set(None);
    }
    Ok(())
}
