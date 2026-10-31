//! Резерв переписи :has() живёт вместе с общими паспортами области.

use crate::budget::{
    LiveReservation,
    admission::{Admission, Error},
};
use crate::style::select::{Ancestor, Spot};
use markup5ever_rcdom::{Handle, NodeData};
use std::mem::size_of;

pub(super) fn level(
    children: &[Handle],
    admission: &Admission,
    tree: &crate::dom::tree_index::TreeIndex,
) -> Result<Option<LiveReservation>, Error> {
    // Покрывает векторы, Rc и таблицу счётчиков типов даже при разных тегах.
    let mut bytes = children
        .len()
        .checked_mul(size_of::<Ancestor>() + size_of::<Spot>() + 192)
        .and_then(|n| n.checked_add(64))
        .ok_or(Error::CapacityOverflow)?;
    for child in children {
        let NodeData::Element { name, attrs, .. } = &child.data else {
            continue;
        };
        let marks = tree
            .has_marks
            .get(&crate::dom::tree_index::NodeKey::of(child))
            .map_or(0, Vec::len);
        let marks = marks
            .checked_mul(size_of::<u64>())
            .ok_or(Error::CapacityOverflow)?;
        bytes = bytes
            .checked_add(name.local.len())
            .and_then(|n| n.checked_add(marks))
            .ok_or(Error::CapacityOverflow)?;
        for attr in attrs.borrow().iter() {
            admission.work(1)?;
            let strings = attr
                .value
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(attr.name.local.len()))
                .and_then(|n| n.checked_add(2 * size_of::<String>()))
                .ok_or(Error::CapacityOverflow)?;
            bytes = bytes.checked_add(strings).ok_or(Error::CapacityOverflow)?;
            if &*attr.name.local == "class" {
                let classes = attr
                    .value
                    .split_whitespace()
                    .count()
                    .checked_mul(2 * size_of::<String>())
                    .ok_or(Error::CapacityOverflow)?;
                bytes = bytes.checked_add(classes).ok_or(Error::CapacityOverflow)?;
            }
        }
    }
    admission.storage(bytes)
}
