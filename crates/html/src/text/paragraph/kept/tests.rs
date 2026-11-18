//! Tests of the kept paragraph's atom paths and slot (`super`).

use super::*;

#[test]
fn atom_paths_are_ranges_of_one_buffer() {
    let atoms = KeptAtoms {
        keys: vec![7, 9, 11],
        path: vec![0, 2, 1, 5, 3],
        ends: vec![2, 4, 5],
    };
    assert_eq!(atoms.path_of(0), &[0, 2]);
    assert_eq!(atoms.path_of(1), &[1, 5]);
    assert_eq!(atoms.path_of(2), &[3]);
}

#[test]
fn an_empty_slot_is_not_filled() {
    let slot = KeptSlot::default();
    assert!(!slot.is_filled());
    assert!(slot.take().is_none());
}
