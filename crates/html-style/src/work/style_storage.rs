//! Живые заголовки и уникальные payload-группы стиля в потоке документа.
//! Счётчики не сбрасываются между фазами: снимок отражает удерживаемые владельцы.
//! Вложенные строки/векторы и Rc control blocks сюда не входят; их учитывает allocator.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub computed_headers: u64,
    pub inherited_headers: u64,
    pub group_allocations: u64,
    pub group_payload_bytes: u64,
}
thread_local! {
    static LIVE: Cell<Snapshot> = const { Cell::new(Snapshot {
        computed_headers: 0, inherited_headers: 0,
        group_allocations: 0, group_payload_bytes: 0,
    }) };
}
pub fn snapshot() -> Snapshot {
    LIVE.with(Cell::get)
}
pub(crate) fn header<const INHERITED: bool>(acquire: bool) {
    LIVE.with(|live| {
        let mut s = live.get();
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
        live.set(s);
    });
}
pub(crate) fn group<T>(acquire: bool) {
    LIVE.with(|live| {
        let mut s = live.get();
        if acquire {
            s.group_allocations += 1;
            s.group_payload_bytes += size_of::<T>() as u64;
        } else {
            s.group_allocations -= 1;
            s.group_payload_bytes -= size_of::<T>() as u64;
        }
        live.set(s);
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::computed::{Computed, StyleGroup};
    #[test]
    fn shared_owners_count_one_payload_until_cow_and_release_balances() {
        let before = snapshot();
        {
            let base = StyleGroup::new([0_u64; 3]);
            let mut copy = base.clone();
            let shared = snapshot();
            assert_eq!(shared.group_allocations, before.group_allocations + 1);
            assert_eq!(shared.group_payload_bytes, before.group_payload_bytes + 24);
            copy[0] = 1;
            assert_eq!(snapshot().group_allocations, shared.group_allocations + 1);
            assert_eq!(
                snapshot().group_payload_bytes,
                shared.group_payload_bytes + 24
            );
            drop(base);
            assert_eq!(snapshot(), shared);
        }
        assert_eq!(snapshot(), before);
    }
    #[test]
    fn header_clones_are_separate_from_unique_groups() {
        drop(Computed::default());
        let before = snapshot();
        {
            let base = Computed::default();
            let shared = snapshot();
            let copy = base.clone();
            let cloned = snapshot();
            assert_eq!(cloned.computed_headers, shared.computed_headers + 1);
            assert_eq!(cloned.inherited_headers, shared.inherited_headers);
            assert_eq!(cloned.group_allocations, shared.group_allocations);
            drop(copy);
            assert_eq!(snapshot(), shared);
        }
        assert_eq!(snapshot(), before);
    }
}
