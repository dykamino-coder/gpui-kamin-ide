//! Занятое фрагментацией место перед строкой одного локального потока абзаца.
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(crate) struct LinePush {
    pub(crate) at: f32,
    pub(crate) line: usize,
    pub(crate) growth: Rc<RefCell<Vec<f32>>>,
}
impl PartialEq for LinePush {
    fn eq(&self, other: &Self) -> bool {
        self.line == other.line && Rc::ptr_eq(&self.growth, &other.growth)
    }
}
impl LinePush {
    pub(crate) fn grow(&self, amount: f32) -> bool {
        if !amount.is_finite() || amount <= 0.0 {
            return false;
        }
        let mut growth = self.growth.borrow_mut();
        let count = growth.len().max(self.line + 1);
        growth.resize(count, 0.0);
        growth[self.line] += amount;
        true
    }
}
