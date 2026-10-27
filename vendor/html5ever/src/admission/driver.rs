//! A terminal driver never sends EOF or finalizes a rejected sink.

use super::{Admission, Operation};
use crate::tree_builder::TreeSink;

/// The fallible entry point deliberately has no TendrilSink implementation:
/// its error cannot accidentally become a successful partial document.
pub struct BoundedParser<S: TreeSink, A: Admission<S>> {
    pub tokenizer: crate::tokenizer::Tokenizer<crate::tree_builder::TreeBuilder<S::Handle, S, A>>,
    pub input_buffer: crate::buffer_queue::BufferQueue,
}

pub fn parse_document<S: TreeSink, A: Admission<S>>(
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

impl<S: TreeSink, A: Admission<S>> BoundedParser<S, A> {
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
        self.tokenizer
            .sink
            .admission
            .finish(self.tokenizer.sink.sink)
    }
}
