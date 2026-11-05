//! Владение отложенным элементом восстанавливается при отмене продолжения.
use std::cell::RefCell;
pub(super) struct ElementGuard<'a, T> {
    slot: &'a RefCell<Option<T>>,
    value: Option<T>,
}
impl<'a, T> ElementGuard<'a, T> {
    pub(super) fn take(slot: &'a RefCell<Option<T>>) -> Self {
        Self {
            slot,
            value: slot.take(),
        }
    }
    pub(super) fn as_mut(&mut self) -> Option<&mut T> {
        self.value.as_mut()
    }
}
impl<T> Drop for ElementGuard<'_, T> {
    fn drop(&mut self) {
        assert!(
            self.slot.borrow().is_none(),
            "reentrant fragment construction"
        );
        self.slot.replace(self.value.take());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_restores_the_same_element() {
        let slot = RefCell::new(Some(Box::new(17)));
        let pointer = slot.borrow().as_deref().unwrap() as *const i32;
        {
            let mut guard = ElementGuard::take(&slot);
            **guard.as_mut().unwrap() = 23;
            assert!(slot.borrow().is_none());
        }
        assert_eq!(slot.borrow().as_deref(), Some(&23));
        assert_eq!(slot.borrow().as_deref().unwrap() as *const i32, pointer);
    }
}
