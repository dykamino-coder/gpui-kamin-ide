//! Document-local fallible admission for the canonical XML parser.
//!
//! Operational failure is terminal: the parser core is destroyed before returning
//! the typed failure, and subsequent calls neither read nor invoke admission.

use super::{parser::PullParser, Error, ParserConfig2};
use crate::common::{Position, TextPosition};
use std::{any::Any, cell::RefCell, io::Read, rc::Rc, sync::Arc};

pub(crate) mod control;
mod events;
mod storage;
pub(crate) mod tables;

use control::{Control, Shared};
pub use events::{Attribute, Event, Name, Namespace};
pub use storage::{Storage, Text};

/// Opaque reservation owned by the corresponding backing allocation.
pub type Lease = Option<Arc<dyn Any + Send + Sync>>;

/// Per-document policy. Implementations must not share mutable global counters.
pub trait Admission {
    /// Typed resource refusal.
    type Error: Clone;
    /// Admit actual input, scanning, comparison, movement and dispatch work.
    fn work(&self, amount: usize) -> Result<(), Self::Error>;
    /// Admit a name spelling before interning or name-table insertion.
    fn name(&self, name: &str) -> Result<(), Self::Error>;
    /// Reserve the complete next allocation while the previous one is still live.
    fn storage(&self, bytes: usize) -> Result<Lease, Self::Error>;
}

/// A syntax or operational outcome; operational refusal never becomes XML syntax.
#[derive(Debug, Clone)]
pub enum Failure<E> {
    /// XML syntax or underlying input error.
    Xml(Error),
    /// Policy refused a resource request.
    Admission(E),
    /// Fallible allocation refused storage.
    Allocation,
    /// Checked capacity arithmetic overflowed.
    CapacityOverflow,
}

/// An abortable owner of a single canonical parser and its per-document policy.
pub struct AbortableReader<R: Read, A: Admission> {
    source: R,
    parser: Option<PullParser>,
    shared: Option<Rc<Shared<A>>>,
    position: TextPosition,
    ended: bool,
}

impl<R: Read, A: Admission + 'static> AbortableReader<R, A> {
    /// All parser constructor storage is admitted before its allocation.
    /// The admission policy replaces the legacy `ParserConfig2` resource caps;
    /// syntax and decoding options retain their canonical behavior.
    pub fn new(
        source: R,
        config: impl Into<ParserConfig2>,
        policy: A,
    ) -> Result<Self, Failure<A::Error>> {
        let core = policy
            .storage(std::mem::size_of::<Shared<A>>())
            .map_err(Failure::Admission)?;
        let shared = Rc::new(Shared {
            policy,
            failure: RefCell::new(None),
            _lease: core,
        });
        let control = Control::new(shared.clone());
        let parser = PullParser::with_control(config.into(), control).map_err(|error| {
            shared
                .failure
                .borrow_mut()
                .take()
                .unwrap_or(Failure::Xml(error))
        })?;
        Ok(Self {
            source,
            parser: Some(parser),
            shared: Some(shared),
            position: TextPosition::new(),
            ended: false,
        })
    }
}

impl<R: Read, A: Admission + 'static> Iterator for AbortableReader<R, A> {
    type Item = Result<Event, Failure<A::Error>>;

    /// Pull an owned event. A terminal failure is followed by `None`.
    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            return None;
        }
        let parser = self.parser.as_mut()?;
        let result = parser.next_owned(&mut self.source);
        self.position = parser.position();
        match result {
            Ok(Event::EndDocument) => {
                self.ended = true;
                self.parser = None;
                self.shared = None;
                Some(Ok(Event::EndDocument))
            }
            Ok(event) => Some(Ok(event)),
            Err(error) => {
                let failure = self
                    .shared
                    .as_ref()
                    .expect("live parser policy")
                    .failure
                    .borrow_mut()
                    .take()
                    .unwrap_or(Failure::Xml(error));
                self.parser = None;
                self.shared = None;
                self.ended = true;
                Some(Err(failure))
            }
        }
    }
}

impl<R: Read, A: Admission> Position for AbortableReader<R, A> {
    fn position(&self) -> TextPosition {
        self.position
    }
}

#[cfg(test)]
mod tests;
