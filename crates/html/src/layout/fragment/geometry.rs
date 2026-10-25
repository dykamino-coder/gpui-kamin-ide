//! Адаптер завершённой геометрии: LayoutId действителен только в своём дереве.
//! Числа normal rect отделены от visual offset; paint применяет translation сам.
use gpui::{Bounds, FinalLayout, LayoutId, LayoutTreeId, Pixels, Window};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) id: LayoutId,
    tree: LayoutTreeId,
}

impl Node {
    pub(crate) fn capture(id: LayoutId, window: &Window) -> Self {
        Self {
            id,
            tree: window.layout_tree_id(),
        }
    }

    pub(crate) fn id_in(self, window: &Window) -> Option<LayoutId> {
        (self.tree == window.layout_tree_id()).then_some(self.id)
    }

    pub(crate) fn read(self, window: &Window) -> Option<FinalLayout> {
        window.final_layout(self.id, self.tree)
    }

    pub(crate) fn bounds(self, window: &Window) -> Option<Bounds<Pixels>> {
        self.read(window).map(|layout| layout.normal)
    }
}

#[derive(Debug)]
pub(crate) struct Records<T>(Rc<RefCell<Vec<(Node, T)>>>);

impl<T> Default for Records<T> {
    fn default() -> Self {
        Self(Rc::default())
    }
}

impl<T> Records<T> {
    pub(crate) fn record(&self, id: LayoutId, value: T, window: &Window) {
        self.0.borrow_mut().push((Node::capture(id, window), value));
    }

    pub(crate) fn reset(&self) {
        self.0.borrow_mut().clear();
    }

    pub(crate) fn enter(&self) -> Scope<T> {
        Scope {
            owner: self.0.clone(),
            saved: std::mem::take(&mut *self.0.borrow_mut()),
        }
    }

    /// Каждое чтение проверяет актуальный final-pass membership, включая предков.
    pub(crate) fn for_each(&self, window: &Window, mut read: impl FnMut(Bounds<Pixels>, &T)) {
        for (node, value) in self.0.borrow().iter() {
            if let Some(bounds) = node.bounds(window) {
                read(bounds, value);
            }
        }
    }
}

pub(crate) struct Scope<T> {
    owner: Rc<RefCell<Vec<(Node, T)>>>,
    saved: Vec<(Node, T)>,
}

impl<T> Drop for Scope<T> {
    fn drop(&mut self) {
        *self.owner.borrow_mut() = std::mem::take(&mut self.saved);
    }
}

#[cfg(test)]
mod tests;
