//! Measurements shared by vertical text and root painting in one document.

use gpui::Pixels;
use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub(crate) struct VerticalMeasures {
    pub(crate) measured: RefCell<HashMap<u64, Pixels>>,
    pub(crate) root_left: RefCell<HashMap<u64, f32>>,
    sequence: RefCell<HashMap<u64, u64>>,
}

impl VerticalMeasures {
    pub(crate) fn begin_frame(&self) {
        self.sequence.borrow_mut().clear();
    }

    pub(crate) fn sequence_key(&self, base: u64) -> u64 {
        let mut sequence = self.sequence.borrow_mut();
        let n = sequence.entry(base).or_insert(0);
        *n += 1;
        base ^ n.wrapping_mul(0x517C_C1B7_2722_0A95)
    }
}

#[cfg(test)]
mod tests {
    use gpui::px;

    #[test]
    fn document_measurements_and_sequence_resets_are_independent() {
        let a = crate::document::state::DocState::default();
        let b = crate::document::state::DocState::default();
        let a = &a.vertical_measures;
        let b = &b.vertical_measures;
        let first = a.sequence_key(7);
        assert_eq!(first, b.sequence_key(7));
        let second = a.sequence_key(7);
        assert_ne!(first, second);
        a.measured.borrow_mut().insert(first, px(20.));
        b.measured.borrow_mut().insert(first, px(40.));
        a.root_left.borrow_mut().insert(1, 12.);
        b.root_left.borrow_mut().insert(1, 30.);
        a.begin_frame();
        assert_eq!(a.sequence_key(7), first);
        assert_eq!(b.sequence_key(7), second);
        assert_eq!(a.measured.borrow()[&first], px(20.));
        assert_eq!(b.measured.borrow()[&first], px(40.));
        assert_eq!(a.root_left.borrow()[&1], 12.);
        assert_eq!(b.root_left.borrow()[&1], 30.);
    }
}
