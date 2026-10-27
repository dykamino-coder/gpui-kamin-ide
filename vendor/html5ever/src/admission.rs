//! Admission precedes tree mutations; exhaustion is terminal, not HTML recovery.

use crate::tree_builder::{NodeOrText, TreeSink};
use crate::{Attribute, QualName};
use std::convert::Infallible;

pub mod deque;
pub mod storage;
use storage::Storage;

/// An operation about to run in the canonical parser. Arguments are borrowed
/// so a rejecting policy never needs to clone or invent DOM handles.
pub enum Operation<'a, H> {
    /// Input is admitted before it enters the tokenizer's queue.
    Input(usize),
    TokenBuffer(&'a crate::tendril::StrTendril, usize),
    /// Logical admission of a unique spelling precedes dynamic interning.
    Name(&'a str),
    TokenAttribute(&'a str, usize),
    Vector {
        capacity: usize,
        required: usize,
        element_size: usize,
    },
    TokenizerStep,
    TreeStep,
    /// Admitted byte/entry work before a bulk copy, comparison or shift.
    Work(usize),
    CreateElement(&'a QualName, &'a [Attribute]),
    CreateComment(&'a str),
    CreatePi(&'a str, &'a str),
    Append(&'a H, &'a NodeOrText<H>),
    BeforeSibling(&'a H, &'a NodeOrText<H>),
    FosterParent(&'a H, &'a H, &'a NodeOrText<H>),
    Doctype(&'a str, &'a str, &'a str),
    Attributes(&'a H, &'a [Attribute]),
    Reparent(&'a H, &'a H),
    Remove(&'a H),
    Pop(&'a H),
    Selectedcontent(&'a H),
    Shadow(&'a H, &'a H, &'a [Attribute]),
    ParseError(&'a str),
    Finish,
}

/// A policy owns its error and any reservations. On failure the driver stops
/// without consuming more input, delivering EOF, or finalizing the sink.
pub trait Admission<S: TreeSink> {
    type Error: Clone;
    fn check(&self, operation: Operation<'_, S::Handle>) -> Result<(), Self::Error>;

    /// Sink implementations with internal allocating work override these
    /// methods, propagating refusal through the same terminal parser outcome.
    fn create_element(
        &self,
        sink: &S,
        name: QualName,
        attrs: Vec<Attribute>,
        flags: crate::interface::ElementFlags,
    ) -> Result<S::Handle, Self::Error> {
        Ok(sink.create_element(name, attrs, flags))
    }
    fn create_comment(
        &self,
        sink: &S,
        text: crate::tendril::StrTendril,
    ) -> Result<S::Handle, Self::Error> {
        Ok(sink.create_comment(text))
    }
    fn create_pi(
        &self,
        sink: &S,
        target: crate::tendril::StrTendril,
        data: crate::tendril::StrTendril,
    ) -> Result<S::Handle, Self::Error> {
        Ok(sink.create_pi(target, data))
    }
    fn append(
        &self,
        sink: &S,
        parent: &S::Handle,
        child: NodeOrText<S::Handle>,
    ) -> Result<(), Self::Error> {
        sink.append(parent, child);
        Ok(())
    }
    fn before_sibling(
        &self,
        sink: &S,
        sibling: &S::Handle,
        child: NodeOrText<S::Handle>,
    ) -> Result<(), Self::Error> {
        sink.append_before_sibling(sibling, child);
        Ok(())
    }
    fn foster_parent(
        &self,
        sink: &S,
        element: &S::Handle,
        previous: &S::Handle,
        child: NodeOrText<S::Handle>,
    ) -> Result<(), Self::Error> {
        sink.append_based_on_parent_node(element, previous, child);
        Ok(())
    }
    fn doctype(
        &self,
        sink: &S,
        name: crate::tendril::StrTendril,
        public: crate::tendril::StrTendril,
        system: crate::tendril::StrTendril,
    ) -> Result<(), Self::Error> {
        sink.append_doctype_to_document(name, public, system);
        Ok(())
    }
    fn attributes(
        &self,
        sink: &S,
        node: &S::Handle,
        attrs: Vec<Attribute>,
    ) -> Result<(), Self::Error> {
        sink.add_attrs_if_missing(node, attrs);
        Ok(())
    }
    fn remove(&self, sink: &S, node: &S::Handle) -> Result<(), Self::Error> {
        sink.remove_from_parent(node);
        Ok(())
    }
    fn reparent(&self, sink: &S, node: &S::Handle, parent: &S::Handle) -> Result<(), Self::Error> {
        sink.reparent_children(node, parent);
        Ok(())
    }
    fn pop(&self, sink: &S, node: &S::Handle) -> Result<(), Self::Error> {
        sink.pop(node);
        Ok(())
    }
    fn selectedcontent(&self, sink: &S, node: &S::Handle) -> Result<(), Self::Error> {
        sink.maybe_clone_an_option_into_selectedcontent(node);
        Ok(())
    }
    fn shadow(
        &self,
        sink: &S,
        host: &S::Handle,
        template: &S::Handle,
        attrs: &[Attribute],
    ) -> Result<bool, Self::Error> {
        Ok(sink.attach_declarative_shadow(host, template, attrs))
    }
    fn parse_error(
        &self,
        sink: &S,
        message: std::borrow::Cow<'static, str>,
    ) -> Result<(), Self::Error> {
        sink.parse_error(message);
        Ok(())
    }
    fn parse_error_fmt(
        &self,
        sink: &S,
        message: std::fmt::Arguments<'_>,
    ) -> Result<(), Self::Error> {
        self.parse_error(sink, std::borrow::Cow::Owned(message.to_string()))
    }
    fn finish(&self, sink: S) -> Result<S::Output, Self::Error>
    where
        S: Sized,
    {
        Ok(sink.finish())
    }

    /// A bounded policy makes the storage unique and reserves its capacity
    /// before the tokenizer can mutate a shared or growing tendril.
    fn reserve_buffer(
        &self,
        buffer: &mut crate::tendril::StrTendril,
        additional: usize,
    ) -> Result<(), Self::Error> {
        self.check(Operation::TokenBuffer(buffer, additional))
    }

    /// A bounded policy overrides this to pair fallible allocation with its
    /// storage reservation. Existing callers retain Vec's allocation contract.
    fn reserve<T>(&self, values: &mut Storage<T>, additional: usize) -> Result<(), Self::Error> {
        if additional > values.capacity() - values.len() {
            self.check(Operation::Vector {
                capacity: values.capacity(),
                required: values.len().saturating_add(additional),
                element_size: std::mem::size_of::<T>(),
            })?;
            values.reserve(additional);
        }
        Ok(())
    }

    fn reserve_deque<T>(
        &self,
        values: &mut deque::StorageDeque<T>,
        additional: usize,
    ) -> Result<(), Self::Error> {
        if additional > values.capacity() - values.len() {
            self.check(Operation::Vector {
                capacity: values.capacity(),
                required: values.len().saturating_add(additional),
                element_size: std::mem::size_of::<T>(),
            })?;
            values.reserve(additional);
        }
        Ok(())
    }
}

/// Existing callers keep their infallible contract and parser behavior.
#[derive(Default)]
pub struct Unbounded;

impl<S: TreeSink> Admission<S> for Unbounded {
    type Error = Infallible;
    fn check(&self, _: Operation<'_, S::Handle>) -> Result<(), Infallible> {
        Ok(())
    }
}

mod driver;
pub use driver::{parse_document, BoundedParser};

#[cfg(test)]
mod storage_tests;
