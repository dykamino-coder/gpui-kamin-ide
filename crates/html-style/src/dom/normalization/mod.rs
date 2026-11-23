//! Ordered tree transforms retain one current result per immutable input list.
//!
//! A context includes every inherited and pass-wide input. Transforms may not
//! read mutable state absent from that context. Each pass owns its own records:
//! a postorder edit must never run over another pass's final output.
use crate::budget::admission::{Admission, Error};
use crate::dom::{Children, Element};
use std::rc::Rc;

mod engine;
mod record;
#[cfg(test)]
mod tests;
use record::Segment;

pub(crate) trait Transform {
    type Context: Clone + PartialEq;

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
        nodes: Children,
        _parent: &Self::Context,
        _admission: &Admission,
    ) -> Result<Children, Error> {
        Ok(nodes)
    }
}

pub(crate) struct Pass<T: Transform> {
    transform: T,
    current: Option<Rc<Segment<T::Context>>>,
}

/// Preparation has no predecessor link and does not change the committed pass.
pub(crate) struct Prepared<'a, T: Transform> {
    pass: &'a mut Pass<T>,
    root: Rc<Segment<T::Context>>,
}

impl<T: Transform> Prepared<'_, T> {
    pub(crate) fn output(&self) -> &Children {
        &self.root.output
    }

    /// All passes and bindings must prepare successfully before publication.
    pub(crate) fn publish(self) {
        self.pass.current = Some(self.root);
    }
}

impl<T: Transform> Pass<T> {
    pub(crate) fn new(transform: T) -> Self {
        Self {
            transform,
            current: None,
        }
    }

    pub(crate) fn prepare(
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
