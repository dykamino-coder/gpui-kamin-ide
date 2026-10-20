//! Inline extent collected by one intrinsic measurement, independent of its parent.

use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct InlineMeasure(Rc<Cell<f32>>);

impl InlineMeasure {
    pub(crate) fn reset(&self) {
        self.0.set(0.0);
    }
    pub(crate) fn report(&self, extent: f32) {
        self.0.set(self.0.get().max(extent));
    }
    pub(crate) fn extent(&self) -> f32 {
        self.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_owner_shares_results_without_contaminating_nested_measurements() {
        let outer = InlineMeasure::default();
        let retained = outer.clone();
        retained.report(20.0);
        let inner = InlineMeasure::default();
        inner.report(100.0);
        assert_eq!(outer.extent(), 20.0);
        assert_eq!(inner.extent(), 100.0);
        outer.reset();
        retained.report(5.0);
        retained.report(2.0);
        assert_eq!(outer.extent(), 5.0);
        assert_eq!(inner.extent(), 100.0);
    }
}
