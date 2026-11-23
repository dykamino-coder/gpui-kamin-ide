//! A pass's admitted output buffer stays charged while any frame holds it.
use super::{Children, Snapshot};
use crate::budget::{LiveReservation, admission::Error};
use std::{fmt, mem, rc::Rc};

#[derive(Default)]
pub(super) struct Storage(Option<Box<LiveReservation>>);

impl fmt::Debug for Storage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Storage").field(&self.0.is_some()).finish()
    }
}

#[cfg(feature = "heap-report")]
impl Storage {
    pub(super) fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        self.0
            .as_ref()
            .map_or(0, |lease| ops.shallow(lease.as_ref()))
    }
}

impl Children {
    pub(crate) fn storage_bytes(count: usize) -> Result<usize, Error> {
        count
            .checked_mul(mem::size_of::<super::Node>())
            .and_then(|bytes| {
                bytes.checked_add(mem::size_of::<Snapshot>() + 2 * mem::size_of::<usize>())
            })
            .and_then(|bytes| bytes.checked_add(mem::size_of::<LiveReservation>()))
            .ok_or(Error::CapacityOverflow)
    }

    pub(crate) fn from_reserved(nodes: Vec<super::Node>, lease: Option<LiveReservation>) -> Self {
        let mut children = Self::from(nodes);
        Rc::get_mut(&mut children.0).expect("new snapshot").storage = Storage(lease.map(Box::new));
        children
    }
}
