//! A pass's admitted output buffer stays charged while any frame holds it.
use super::{Children, Snapshot};
use crate::budget::{
    LiveReservation,
    admission::{Admission, Error},
};
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
    /// Reserve before separating a shared list. Old snapshots and their leases
    /// remain intact if admission or allocation fails.
    pub(crate) fn edit(
        &mut self,
        admission: &Admission,
        additional: usize,
    ) -> Result<&mut Vec<super::Node>, Error> {
        let wanted = self
            .len()
            .checked_add(additional)
            .ok_or(Error::CapacityOverflow)?;
        if Rc::get_mut(&mut self.0).is_some() && self.capacity() >= wanted {
            return Ok(std::ops::DerefMut::deref_mut(self));
        }
        let lease = admission.storage(Self::storage_bytes(wanted)?)?;
        admission.work(self.len())?;
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(wanted)
            .map_err(|_| Error::Allocation)?;
        nodes.extend_from_slice(self);
        *self = Self::from_reserved(nodes, lease);
        Ok(std::ops::DerefMut::deref_mut(self))
    }

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
        Self(Rc::new(Snapshot {
            nodes,
            _storage: Storage(lease.map(Box::new)),
            ..Default::default()
        }))
    }
}
