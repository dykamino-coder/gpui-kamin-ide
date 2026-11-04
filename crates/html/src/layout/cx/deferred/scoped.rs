//! Временное состояние фрагмента восстанавливается при завершении и отмене обхода.

use std::cell::RefCell;

pub(crate) struct Scope<'a, T> {
    cell: &'a RefCell<T>,
    previous: Option<T>,
}

impl<'a, T> Scope<'a, T> {
    pub(crate) fn enter(cell: &'a RefCell<T>, value: T) -> Self {
        Self {
            cell,
            previous: Some(cell.replace(value)),
        }
    }
}

impl<T> Drop for Scope<'_, T> {
    fn drop(&mut self) {
        self.cell
            .replace(self.previous.take().expect("состояние области"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restores_nested_state_and_unwind() {
        let cell = RefCell::new(1);
        let outer = Scope::enter(&cell, 2);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _inner = Scope::enter(&cell, 3);
            assert_eq!(*cell.borrow(), 3);
            panic!("проверка восстановления");
        }));
        assert!(panic.is_err());
        assert_eq!(*cell.borrow(), 2);
        drop(outer);
        assert_eq!(*cell.borrow(), 1);
    }
}
