//! Общая политика parser/sink; logical payload и physical storage имеют разных owners.

use super::names::Names;
use crate::budget::{BudgetKind, RevisionBudget};
use crate::document::FailureReason;
use std::cell::Cell;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

type Lease = markup5ever_rcdom::admission::Lease;

#[derive(Default)]
struct Counts {
    nodes: AtomicU64,
    text: AtomicU64,
    attributes: AtomicU64,
}

struct Logical {
    counts: Arc<Counts>,
    nodes: u64,
    text: AtomicU64,
    attributes: AtomicU64,
}

impl Drop for Logical {
    fn drop(&mut self) {
        self.counts.nodes.fetch_sub(self.nodes, Ordering::Relaxed);
        self.counts
            .text
            .fetch_sub(self.text.load(Ordering::Relaxed), Ordering::Relaxed);
        self.counts
            .attributes
            .fetch_sub(self.attributes.load(Ordering::Relaxed), Ordering::Relaxed);
    }
}

pub(super) struct Policy {
    pub(super) budget: RevisionBudget,
    names: Cell<Names>,
    counts: Arc<Counts>,
}

impl Policy {
    pub(super) fn new(budget: RevisionBudget) -> Self {
        Self {
            budget,
            names: Cell::default(),
            counts: Arc::default(),
        }
    }

    pub(super) fn name(&self, name: &str) -> Result<(), FailureReason> {
        let mut names = self.names.take();
        let result = names.admit(name, &self.budget);
        self.names.set(names);
        result
    }

    pub(super) fn storage(&self, bytes: usize) -> Result<Lease, FailureReason> {
        self.ticket(bytes).map(Some)
    }

    fn ticket(&self, bytes: usize) -> Result<Arc<dyn std::any::Any + Send + Sync>, FailureReason> {
        self.budget
            .document()
            .reserve(BudgetKind::LiveBytes, bytes as u64)
            .map(|ticket| Arc::new(ticket) as Arc<dyn std::any::Any + Send + Sync>)
            .map_err(FailureReason::Budget)
    }

    pub(super) fn work(&self, visits: usize) -> Result<(), FailureReason> {
        self.budget
            .consume(BudgetKind::TraversalVisits, visits as u64)
            .map_err(FailureReason::Budget)
    }

    fn maximum(
        &self,
        kind: BudgetKind,
        current: u64,
        old: u64,
        new: u64,
    ) -> Result<(), FailureReason> {
        let total = current
            .checked_sub(old)
            .and_then(|n| n.checked_add(new))
            .ok_or(FailureReason::CapacityOverflow)?;
        self.budget
            .check_maximum(kind, total)
            .map_err(FailureReason::Budget)
    }

    fn logical(
        &self,
        nodes: u64,
        text: usize,
        attributes: usize,
        previous: &Lease,
    ) -> Result<Lease, FailureReason> {
        let old = previous
            .as_ref()
            .and_then(|lease| lease.downcast_ref::<Logical>());
        let old_text = old.map_or(0, |lease| lease.text.load(Ordering::Relaxed));
        let old_attrs = old.map_or(0, |lease| lease.attributes.load(Ordering::Relaxed));
        self.maximum(
            BudgetKind::DomNodes,
            self.counts.nodes.load(Ordering::Relaxed),
            0,
            nodes,
        )?;
        self.maximum(
            BudgetKind::TextBytes,
            self.counts.text.load(Ordering::Relaxed),
            old_text,
            text as u64,
        )?;
        self.maximum(
            BudgetKind::AttributeBytes,
            self.counts.attributes.load(Ordering::Relaxed),
            old_attrs,
            attributes as u64,
        )?;
        // Мутации парсера однопоточны. Send + Sync позволяет буферам
        // удерживать владельцев при разделении памяти через tendril.
        self.counts.nodes.fetch_add(nodes, Ordering::Relaxed);
        self.counts.text.fetch_add(text as u64, Ordering::Relaxed);
        self.counts
            .attributes
            .fetch_add(attributes as u64, Ordering::Relaxed);
        if let Some(old) = old {
            self.counts
                .text
                .fetch_sub(old.text.swap(0, Ordering::Relaxed), Ordering::Relaxed);
            self.counts
                .attributes
                .fetch_sub(old.attributes.swap(0, Ordering::Relaxed), Ordering::Relaxed);
        }
        Ok(Some(Arc::new(Logical {
            counts: self.counts.clone(),
            nodes,
            text: AtomicU64::new(text as u64),
            attributes: AtomicU64::new(attributes as u64),
        })))
    }

    pub(super) fn buffer(
        &self,
        buffer: &mut html5ever::tendril::StrTendril,
        additional: usize,
    ) -> Result<(), FailureReason> {
        let additional = u32::try_from(additional).map_err(|_| FailureReason::CapacityOverflow)?;
        let copied = buffer.len();
        buffer
            .try_reserve_with(additional, |bytes| {
                self.work(copied)?;
                self.ticket(bytes)
            })
            .map_err(|error| match error {
                html5ever::tendril::ReserveError::Admission(error) => error,
                html5ever::tendril::ReserveError::Allocation(_) => FailureReason::Allocation,
                html5ever::tendril::ReserveError::CapacityOverflow => {
                    FailureReason::CapacityOverflow
                }
            })
    }
}

impl markup5ever_rcdom::admission::Admission for Policy {
    type Error = FailureReason;
    fn storage(&self, bytes: usize) -> Result<Lease, Self::Error> {
        self.storage(bytes)
    }
    fn node(&self, bytes: usize) -> Result<Lease, Self::Error> {
        self.storage(bytes)
    }
    fn node_count(&self) -> Result<Lease, Self::Error> {
        self.logical(1, 0, 0, &None)
    }
    fn payload(&self, text: usize, attributes: usize) -> Result<Lease, Self::Error> {
        self.logical(0, text, attributes, &None)
    }
    fn replace_payload(
        &self,
        text: usize,
        attributes: usize,
        old: &Lease,
    ) -> Result<Lease, Self::Error> {
        self.logical(0, text, attributes, old)
    }
    fn buffer(
        &self,
        buffer: &mut html5ever::tendril::StrTendril,
        additional: usize,
    ) -> Result<(), Self::Error> {
        self.buffer(buffer, additional)
    }
    fn visit(&self) -> Result<(), Self::Error> {
        self.work(1)
    }
    fn work(&self, amount: usize) -> Result<(), Self::Error> {
        self.work(amount)
    }
    fn allocation(&self, _: std::collections::TryReserveError) -> Self::Error {
        FailureReason::Allocation
    }
    fn overflow(&self) -> Self::Error {
        FailureReason::CapacityOverflow
    }
}
