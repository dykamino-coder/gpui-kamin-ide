//! Ordered tree transforms retain one current result per immutable input list.
//!
//! A context includes every inherited and pass-wide input. Transforms may not
//! read mutable state absent from that context. Each pass owns its own records:
//! a postorder edit must never run over another pass's final output.
use crate::budget::admission::{Admission, Error};
use crate::dom::{Children, Element};
use std::rc::Rc;

mod engine;
mod frame;
mod owned;
mod record;
#[cfg(test)]
mod tests;
pub(super) use owned::apply;
use record::Segment;

pub trait Transform {
    type Context: Clone + PartialEq;

    /// Runs once on a list miss, after retaining its immutable input key.
    fn before_list(
        &self,
        _nodes: &mut Children,
        _parent: &Self::Context,
        _admission: &Admission,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// A skipped child remains unchanged, without walking or recording descendants.
    /// The decision must depend only on this list and its complete context.
    fn visit_child(&self, _nodes: &Children, _index: usize, _parent: &Self::Context) -> bool {
        true
    }

    fn enter(
        &self,
        element: &mut Element,
        parent: &Self::Context,
        admission: &Admission,
    ) -> Result<Self::Context, Error>;

    fn leave(
        &self,
        _element: &mut Element,
        _parent: &Self::Context,
        _admission: &Admission,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// Sequence edits run after children of this pass, including at the root.
    /// Allocations performed by a transform remain its admission obligation.
    fn sequence(
        &self,
        _nodes: &mut Children,
        _parent: &Self::Context,
        _admission: &Admission,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// Called after a child is complete, before entering its next sibling.
    /// Edits may reparent that child into an unvisited sibling. The returned
    /// cursor identifies the next child to enter in the edited list.
    fn after_child(
        &self,
        _nodes: &mut Children,
        index: usize,
        _parent: &Self::Context,
        _admission: &Admission,
    ) -> Result<usize, Error> {
        Ok(index + 1)
    }
}

pub struct Pass<T: Transform> {
    transform: T,
    current: Option<Rc<Segment<T::Context>>>,
}

/// Preparation has no predecessor link and does not change the committed pass.
pub struct Prepared<'a, T: Transform> {
    pass: &'a mut Pass<T>,
    root: Rc<Segment<T::Context>>,
}

impl<T: Transform> Prepared<'_, T> {
    pub fn output(&self) -> &Children {
        &self.root.output
    }

    /// All passes and bindings must prepare successfully before publication.
    pub fn publish(self) {
        self.pass.current = Some(self.root);
    }
}

impl<T: Transform> Pass<T> {
    pub fn new(transform: T) -> Self {
        Self {
            transform,
            current: None,
        }
    }

    pub fn prepare(
        &mut self,
        input: &Children,
        context: T::Context,
        admission: &Admission,
    ) -> Result<Prepared<'_, T>, Error> {
        let root = engine::prepare(
            &self.transform,
            input,
            context,
            self.current.clone(),
            admission,
        )?;
        Ok(Prepared { pass: self, root })
    }
}
