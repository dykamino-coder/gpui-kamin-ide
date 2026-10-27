use super::{Admission, Failure, Lease};
use crate::common::TextPosition;
use crate::reader::{Error, ErrorKind};
use std::{cell::RefCell, rc::Rc};

pub(super) trait Checks {
    fn work(&self, amount: usize) -> Result<(), ()>;
    fn name(&self, name: &str) -> Result<(), ()>;
    fn storage(&self, bytes: usize) -> Result<Lease, ()>;
    fn allocation(&self);
    fn overflow(&self);
}

pub(super) struct Shared<A: Admission> {
    pub policy: A,
    pub failure: RefCell<Option<Failure<A::Error>>>,
    pub _lease: Lease,
}

impl<A: Admission> Shared<A> {
    fn latch<T>(&self, request: impl FnOnce() -> Result<T, A::Error>) -> Result<T, ()> {
        if self.failure.borrow().is_some() {
            return Err(());
        }
        request().map_err(|error| *self.failure.borrow_mut() = Some(Failure::Admission(error)))
    }
}

impl<A: Admission> Checks for Shared<A> {
    fn work(&self, amount: usize) -> Result<(), ()> {
        self.latch(|| self.policy.work(amount))
    }
    fn name(&self, name: &str) -> Result<(), ()> {
        self.latch(|| self.policy.name(name))
    }
    fn storage(&self, bytes: usize) -> Result<Lease, ()> {
        self.latch(|| self.policy.storage(bytes))
    }
    fn allocation(&self) {
        self.failure.borrow_mut().get_or_insert(Failure::Allocation);
    }
    fn overflow(&self) {
        self.failure
            .borrow_mut()
            .get_or_insert(Failure::CapacityOverflow);
    }
}

/// A parser-local handle; legacy parsing has no policy and no shared allocation.
#[derive(Clone, Default)]
pub(crate) struct Control(Option<Rc<dyn Checks>>);

impl Control {
    pub(super) fn new<A: Admission + 'static>(shared: Rc<Shared<A>>) -> Self {
        Self(Some(shared))
    }
    pub(crate) fn bounded(&self) -> bool {
        self.0.is_some()
    }
    pub(crate) fn stopped(&self) -> Error {
        Error {
            pos: TextPosition::new(),
            kind: ErrorKind::Syntax("XML admission stopped".into()),
        }
    }
    pub(crate) fn work(&self, amount: usize) -> Result<(), Error> {
        self.0
            .as_ref()
            .map_or(Ok(()), |c| c.work(amount))
            .map_err(|()| self.stopped())
    }
    pub(crate) fn name(&self, name: &str) -> Result<(), Error> {
        self.0
            .as_ref()
            .map_or(Ok(()), |c| c.name(name))
            .map_err(|()| self.stopped())
    }
    pub(crate) fn storage(&self, bytes: usize) -> Result<Lease, Error> {
        self.0
            .as_ref()
            .map_or(Ok(None), |c| c.storage(bytes))
            .map_err(|()| self.stopped())
    }
    pub(crate) fn allocation(&self) -> Error {
        if let Some(c) = &self.0 {
            c.allocation();
        }
        self.stopped()
    }
    pub(crate) fn overflow(&self) -> Error {
        if let Some(c) = &self.0 {
            c.overflow();
        }
        self.stopped()
    }
}
