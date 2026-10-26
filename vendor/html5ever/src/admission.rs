//! Admission precedes tree mutations; exhaustion is terminal, not HTML recovery.

use crate::tree_builder::{NodeOrText, TreeSink};
use crate::{Attribute, QualName};
use std::convert::Infallible;

/// An operation about to run in the canonical parser. Arguments are borrowed
/// so a rejecting policy never needs to clone or invent DOM handles.
pub enum Operation<'a, H> {
    /// Input is admitted before it enters the tokenizer's queue.
    Input(usize),
    TokenBuffer(&'a crate::tendril::StrTendril, usize),
    TokenAttribute(&'a str, usize),
    Vector {
        capacity: usize,
        required: usize,
        element_size: usize,
    },
    TokenizerStep,
    TreeStep,
    CreateElement(&'a QualName, &'a [Attribute]),
    CreateComment(&'a str),
    CreatePi(&'a str, &'a str),
    Append(&'a H, &'a NodeOrText<H>),
    BeforeSibling(&'a H, &'a NodeOrText<H>),
    FosterParent(&'a H, &'a H, &'a NodeOrText<H>),
    Doctype(&'a str, &'a str, &'a str),
    Attributes(&'a H, &'a [Attribute]),
    Reparent(&'a H, &'a H),
    Pop(&'a H),
    Shadow(&'a H, &'a H, &'a [Attribute]),
    ParseError(&'a str),
    Finish,
}

/// A policy owns its error and any reservations. On failure the driver stops
/// without consuming more input, delivering EOF, or finalizing the sink.
pub trait Admission<H> {
    type Error: Clone;
    fn check(&self, operation: Operation<'_, H>) -> Result<(), Self::Error>;

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
    fn reserve<T>(&self, values: &mut Vec<T>, additional: usize) -> Result<(), Self::Error> {
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
        values: &mut std::collections::VecDeque<T>,
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

impl<H> Admission<H> for Unbounded {
    type Error = Infallible;
    fn check(&self, _: Operation<'_, H>) -> Result<(), Infallible> {
        Ok(())
    }
}

/// The fallible entry point deliberately has no TendrilSink implementation:
/// its error cannot accidentally become a successful partial document.
pub struct BoundedParser<S: TreeSink, A: Admission<S::Handle>> {
    pub tokenizer: crate::tokenizer::Tokenizer<crate::tree_builder::TreeBuilder<S::Handle, S, A>>,
    pub input_buffer: crate::buffer_queue::BufferQueue,
}

pub fn parse_document<S: TreeSink, A: Admission<S::Handle>>(
    sink: S,
    admission: A,
    opts: crate::ParseOpts,
) -> BoundedParser<S, A> {
    let tree = crate::tree_builder::TreeBuilder::with_admission(sink, opts.tree_builder, admission);
    BoundedParser {
        tokenizer: crate::tokenizer::Tokenizer::new(tree, opts.tokenizer),
        input_buffer: Default::default(),
    }
}

impl<S: TreeSink, A: Admission<S::Handle>> BoundedParser<S, A> {
    pub fn process(&mut self, input: crate::tendril::StrTendril) -> Result<(), A::Error> {
        self.tokenizer.sink.result()?;
        if !self.tokenizer.sink.record(
            self.tokenizer
                .sink
                .admission
                .check(Operation::Input(input.len())),
        ) {
            return self.tokenizer.sink.result();
        }
        self.input_buffer.push_back(input);
        self.run()
    }

    /// Admission also precedes the driver's input copy.
    pub fn process_str(&mut self, input: &str) -> Result<(), A::Error> {
        self.tokenizer.sink.result()?;
        if !self.tokenizer.sink.record(
            self.tokenizer
                .sink
                .admission
                .check(Operation::Input(input.len())),
        ) {
            return self.tokenizer.sink.result();
        }
        let mut buffer = crate::tendril::StrTendril::new();
        if !self.tokenizer.sink.record(
            self.tokenizer
                .sink
                .admission
                .reserve_buffer(&mut buffer, input.len()),
        ) {
            return self.tokenizer.sink.result();
        }
        buffer.push_slice(input);
        self.input_buffer.push_back(buffer);
        self.run()
    }

    fn run(&self) -> Result<(), A::Error> {
        loop {
            let result = self.tokenizer.feed(&self.input_buffer);
            self.tokenizer.sink.result()?;
            if matches!(result, markup5ever::TokenizerResult::Done) {
                return Ok(());
            }
        }
    }

    pub fn finish(self) -> Result<S::Output, A::Error> {
        self.tokenizer.sink.result()?;
        self.run()?;
        self.tokenizer.end();
        self.tokenizer.sink.result()?;
        self.tokenizer.sink.admission.check(Operation::Finish)?;
        Ok(self.tokenizer.sink.sink.finish())
    }
}
