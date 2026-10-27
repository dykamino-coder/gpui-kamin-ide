//! html5ever работает с единственной политикой tokenizer и RcDom.

use super::{overhead::QUEUE_OVERHEAD, policy::Policy};
use crate::document::FailureReason;
use html5ever::admission::{
    Admission, Operation,
    deque::StorageDeque,
    storage::{ReserveError, Storage},
};
use html5ever::interface::ElementFlags;
use html5ever::tendril::StrTendril;
use html5ever::tree_builder::NodeOrText;
use html5ever::{Attribute, QualName};
use markup5ever_rcdom::{Handle, RcDom};
use std::borrow::Cow;

fn reserve_error(error: ReserveError<FailureReason>) -> FailureReason {
    match error {
        ReserveError::Admission(error) => error,
        ReserveError::Allocation(_) => FailureReason::Allocation,
        ReserveError::Overflow => FailureReason::CapacityOverflow,
    }
}

impl Admission<RcDom> for &Policy {
    type Error = FailureReason;

    fn check(&self, operation: Operation<'_, Handle>) -> Result<(), Self::Error> {
        match operation {
            Operation::Name(name) => self.name(name),
            Operation::Input(bytes) => self.work(bytes),
            Operation::Work(visits) => self.work(visits),
            Operation::TokenAttribute(name, value) => {
                let bytes = name
                    .len()
                    .checked_add(value)
                    .ok_or(FailureReason::CapacityOverflow)?;
                self.budget
                    .check_maximum(crate::budget::BudgetKind::AttributeBytes, bytes as u64)
                    .map_err(FailureReason::Budget)
            }
            _ => self.work(1),
        }
    }

    fn reserve_buffer(
        &self,
        buffer: &mut StrTendril,
        additional: usize,
    ) -> Result<(), Self::Error> {
        self.buffer(buffer, additional)
    }
    fn reserve<T>(&self, values: &mut Storage<T>, additional: usize) -> Result<(), Self::Error> {
        let moved = values.len();
        values
            .try_reserve_with(additional, |bytes| {
                self.work(moved)?;
                self.storage(bytes)
            })
            .map_err(reserve_error)
    }
    fn reserve_deque<T>(
        &self,
        values: &mut StorageDeque<T>,
        additional: usize,
    ) -> Result<(), Self::Error> {
        let moved = values.len();
        values
            .try_reserve_with(additional, |bytes| {
                self.work(moved)?;
                self.storage(bytes)
            })
            .map_err(reserve_error)
    }
    fn create_element(
        &self,
        sink: &RcDom,
        name: QualName,
        attrs: Vec<Attribute>,
        flags: ElementFlags,
    ) -> Result<Handle, Self::Error> {
        sink.try_create_element(name, attrs, flags, *self)
    }
    fn create_comment(&self, sink: &RcDom, text: StrTendril) -> Result<Handle, Self::Error> {
        sink.try_create_comment(text, *self)
    }
    fn create_pi(
        &self,
        sink: &RcDom,
        target: StrTendril,
        data: StrTendril,
    ) -> Result<Handle, Self::Error> {
        sink.try_create_pi(target, data, *self)
    }
    fn append(
        &self,
        sink: &RcDom,
        parent: &Handle,
        child: NodeOrText<Handle>,
    ) -> Result<(), Self::Error> {
        sink.try_append(parent, child, *self)
    }
    fn before_sibling(
        &self,
        sink: &RcDom,
        sibling: &Handle,
        child: NodeOrText<Handle>,
    ) -> Result<(), Self::Error> {
        sink.try_before_sibling(sibling, child, *self)
    }
    fn foster_parent(
        &self,
        sink: &RcDom,
        element: &Handle,
        previous: &Handle,
        child: NodeOrText<Handle>,
    ) -> Result<(), Self::Error> {
        sink.try_foster_parent(element, previous, child, *self)
    }
    fn doctype(
        &self,
        sink: &RcDom,
        name: StrTendril,
        public: StrTendril,
        system: StrTendril,
    ) -> Result<(), Self::Error> {
        sink.try_doctype(name, public, system, *self)
    }
    fn attributes(
        &self,
        sink: &RcDom,
        node: &Handle,
        attrs: Vec<Attribute>,
    ) -> Result<(), Self::Error> {
        sink.try_attributes(node, attrs, *self)
    }
    fn remove(&self, sink: &RcDom, node: &Handle) -> Result<(), Self::Error> {
        sink.try_remove(node, *self)
    }
    fn reparent(&self, sink: &RcDom, node: &Handle, parent: &Handle) -> Result<(), Self::Error> {
        sink.try_reparent(node, parent, *self)
    }
    fn pop(&self, sink: &RcDom, node: &Handle) -> Result<(), Self::Error> {
        sink.try_pop(node, *self)
    }
    fn selectedcontent(&self, sink: &RcDom, node: &Handle) -> Result<(), Self::Error> {
        sink.try_selectedcontent(node, *self)
    }
    fn parse_error(&self, sink: &RcDom, message: Cow<'static, str>) -> Result<(), Self::Error> {
        sink.try_parse_error(message, *self)
    }
    fn parse_error_fmt(
        &self,
        sink: &RcDom,
        message: std::fmt::Arguments<'_>,
    ) -> Result<(), Self::Error> {
        sink.try_parse_error_fmt(message, *self)
    }
    fn finish(&self, sink: RcDom) -> Result<RcDom, Self::Error> {
        sink.try_finish(*self)
    }
}

pub(super) fn parse(source: &str, policy: &Policy) -> Result<RcDom, FailureReason> {
    let _queues = policy.storage(QUEUE_OVERHEAD)?;
    let dom = RcDom::try_new(policy)?;
    let mut parser = html5ever::admission::parse_document(dom, policy, Default::default());
    parser.process_str(source)?;
    parser.finish()
}
