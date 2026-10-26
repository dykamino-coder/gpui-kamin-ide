//! Terminal operational failure is distinct from XML syntax and reader EOF.

use super::{Error, ParserConfig2, XmlEvent, parser::PullParser};
use crate::common::{Position, TextPosition};
use std::io::Read;

/// Canonical parser work, including characters from queued entity expansion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    /// One canonical parser dispatch or pending-event delivery.
    ParserStep,
    /// One lexer character step, including queued entity expansion.
    LexerStep,
}

/// A policy belongs to one driver; rejection must precede the operation.
pub trait Admission {
    /// The caller-owned operational failure, preserved without string conversion.
    type Error: Clone;
    /// Admit before the parser or lexer performs this operation.
    fn check(&mut self, operation: Operation) -> Result<(), Self::Error>;
}

/// Syntax/IO failures and operational admission failures remain distinguishable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Failure<E> {
    /// The canonical XML parser rejected the source.
    Xml(Error),
    /// The policy rejected further work. No EOF recovery is performed.
    Admission(E),
}

/// Opt-in fallible driver. Rejection latches the first typed policy error;
/// subsequent calls perform no parsing, reads, callbacks, or EOF recovery.
pub struct AbortableReader<R: Read, A: Admission> {
    source: R,
    parser: PullParser,
    failure: Option<Failure<A::Error>>,
    admission: A,
}

impl<R: Read, A: Admission> AbortableReader<R, A> {
    /// Construct a driver using the same parser configuration as EventReader.
    pub fn new(source: R, config: impl Into<ParserConfig2>, admission: A) -> Self {
        Self {
            source,
            parser: PullParser::new(config),
            failure: None,
            admission,
        }
    }

    /// Return the next event, or the first terminal failure on every later call.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<XmlEvent, Failure<A::Error>> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let mut rejected = None;
        let policy = &mut self.admission;
        let result =
            self.parser
                .next_with_admission(&mut self.source, &mut |operation| match policy
                    .check(operation)
                {
                    Ok(()) => true,
                    Err(error) => {
                        rejected = Some(error);
                        false
                    },
                });
        let result = match result {
            Some(Ok(event)) => return Ok(event),
            Some(Err(error)) => Failure::Xml(error),
            None => Failure::Admission(rejected.expect("an abort records its policy error")),
        };
        self.failure = Some(result.clone());
        Err(result)
    }

    /// Borrow the reader without consuming further bytes.
    pub fn source(&self) -> &R {
        &self.source
    }
    /// Inspect the policy and its accounting.
    pub fn admission(&self) -> &A {
        &self.admission
    }
    /// Borrow the doctype already recognized by the canonical parser.
    pub fn doctype(&self) -> Option<&str> {
        self.parser.doctype()
    }
}

impl<R: Read, A: Admission> Position for AbortableReader<R, A> {
    fn position(&self) -> TextPosition {
        self.parser.position()
    }
}

#[cfg(test)]
mod tests;
