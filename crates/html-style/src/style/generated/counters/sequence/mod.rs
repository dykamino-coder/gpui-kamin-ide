//! Persistent indexed sequences for generated-content entry contexts.
use std::rc::Rc;

type Link<T> = Option<Rc<Node<T>>>;
#[derive(Clone)]
struct Node<T: Clone> {
    value: T,
    left: Link<T>,
    right: Link<T>,
    len: usize,
    height: u8,
}
#[derive(Clone)]
pub(super) struct Sequence<T: Clone>(Link<T>);
impl<T: Clone> Default for Sequence<T> {
    fn default() -> Self {
        Self(None)
    }
}
mod tree;
use tree::{insert, len, remove, replace};

impl<T: Clone> Sequence<T> {
    pub fn len(&self) -> usize {
        len(&self.0)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }
    pub fn get(&self, mut index: usize) -> Option<&T> {
        let mut root = self.0.as_deref();
        while let Some(n) = root {
            let count = len(&n.left);
            match index.cmp(&count) {
                std::cmp::Ordering::Less => root = n.left.as_deref(),
                std::cmp::Ordering::Greater => {
                    index -= count + 1;
                    root = n.right.as_deref();
                }
                std::cmp::Ordering::Equal => return Some(&n.value),
            }
        }
        None
    }
    pub fn last(&self) -> Option<&T> {
        self.len().checked_sub(1).and_then(|i| self.get(i))
    }
    pub fn insert(&mut self, index: usize, value: T) {
        assert!(index <= self.len());
        self.0 = Some(insert(self.0.take(), index, value));
    }
    pub fn push(&mut self, value: T) {
        self.insert(self.len(), value);
    }
    pub fn set(&mut self, index: usize, value: T) {
        assert!(index < self.len());
        replace(self.0.as_mut().expect("index"), index, value);
    }
    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len());
        let (root, value) = remove(self.0.take().expect("index"), index);
        self.0 = root;
        value
    }
    pub fn pop(&mut self) -> Option<T> {
        self.len().checked_sub(1).map(|i| self.remove(i))
    }
    pub fn truncate(&mut self, count: usize) {
        while self.len() > count {
            self.pop();
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        (0..self.len()).map(|i| self.get(i).expect("index"))
    }
}
impl<T: Clone + PartialEq> PartialEq for Sequence<T> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) if Rc::ptr_eq(a, b) => true,
            _ => self.len() == other.len() && self.iter().eq(other.iter()),
        }
    }
}
impl<T: Clone + std::fmt::Debug> std::fmt::Debug for Sequence<T> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests;
