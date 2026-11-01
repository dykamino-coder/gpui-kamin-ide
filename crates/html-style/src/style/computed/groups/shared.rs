//! Общий владелец неизменяемой группы: публикация делит данные, запись отделяет группу.

use std::ops::{Deref, DerefMut};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct StyleGroup<T>(Rc<T>);

impl<T> StyleGroup<T> {
    pub fn new(value: T) -> Self {
        Self(Rc::new(value))
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<T: Default> Default for StyleGroup<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: PartialEq> PartialEq for StyleGroup<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other) || *self.0 == *other.0
    }
}

impl<T> Deref for StyleGroup<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> DerefMut for StyleGroup<T> {
    fn deref_mut(&mut self) -> &mut T {
        if Rc::strong_count(&self.0) > 1 {
            crate::work::add!(StyleGroupClones, 1);
            crate::work::add!(StyleGroupCloneBytes, size_of::<T>());
        }
        Rc::make_mut(&mut self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_records_detach_only_on_write() {
        let base = StyleGroup::new(vec![1, 2, 3]);
        let mut delta = base.clone();
        assert!(base.ptr_eq(&delta));
        delta.push(4);
        assert!(!base.ptr_eq(&delta));
        assert_eq!(&*base, &[1, 2, 3]);
        assert_eq!(&*delta, &[1, 2, 3, 4]);
    }
}
