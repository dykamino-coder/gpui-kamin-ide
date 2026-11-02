//! Журнал живых стилей одного документа; владельцы payload удерживают его до освобождения.
//! Вложенные буферы и Rc control blocks учитывает общий allocator отдельно.
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub computed_headers: u64,
    pub inherited_headers: u64,
    pub group_allocations: u64,
    pub group_payload_bytes: u64,
}
impl Snapshot {
    pub fn computed_header_bytes(self) -> u64 {
        self.computed_headers * size_of::<crate::style::computed::Computed>() as u64
    }
    pub fn inherited_header_bytes(self) -> u64 {
        self.inherited_headers * size_of::<crate::style::computed::InheritedStyle>() as u64
    }
}
/// Явный владелец наблюдения, независимый от сессии и времени жизни документа.
#[derive(Clone, Debug, Default)]
pub struct Owner(Rc<Cell<Snapshot>>);
impl Owner {
    pub fn snapshot(&self) -> Snapshot {
        self.0.get()
    }
    pub(crate) fn header<const INHERITED: bool>(&self, acquire: bool) {
        let mut s = self.snapshot();
        let count = if INHERITED {
            &mut s.inherited_headers
        } else {
            &mut s.computed_headers
        };
        if acquire {
            *count += 1;
        } else {
            *count -= 1;
        }
        self.0.set(s);
    }
    fn group(&self, bytes: u64, acquire: bool) {
        let mut s = self.snapshot();
        if acquire {
            s.group_allocations += 1;
            s.group_payload_bytes += bytes;
        } else {
            s.group_allocations -= 1;
            s.group_payload_bytes -= bytes;
        }
        self.0.set(s);
    }
}
/// Один token на уникальный payload, а не на каждую Rc-ручку.
#[derive(Debug, Default)]
pub(crate) struct Allocation(Option<(Owner, u64)>);
impl Allocation {
    pub(crate) fn new<T>(owner: &Owner) -> Self {
        let bytes = size_of::<T>() as u64;
        owner.group(bytes, true);
        Self(Some((owner.clone(), bytes)))
    }
}
impl Clone for Allocation {
    fn clone(&self) -> Self {
        if let Some((owner, bytes)) = &self.0 {
            owner.group(*bytes, true);
        }
        Self(self.0.clone())
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        if let Some((owner, bytes)) = &self.0 {
            owner.group(*bytes, false);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::StyleGroup;
    #[test]
    fn owners_isolate_cow_and_balance_after_document_lifetime() {
        let owner = Owner::default();
        let other = Owner::default();
        let mut base = StyleGroup::new([0_u64; 3]);
        base.track(&owner);
        let mut copy = base.clone();
        assert_eq!(owner.snapshot().group_allocations, 1);
        assert_eq!(owner.snapshot().group_payload_bytes, 24);
        copy[0] = 1;
        assert_eq!(owner.snapshot().group_allocations, 2);
        assert_eq!(other.snapshot(), Snapshot::default());
        drop(base);
        assert_eq!(owner.snapshot().group_allocations, 1);
        drop(copy);
        assert_eq!(owner.snapshot(), Snapshot::default());
    }
}
