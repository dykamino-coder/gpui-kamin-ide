//! Общие дети элемента: копия коробки не копирует поддерево и не углубляет drop.

use super::Node;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// Дети элемента с копированием при изменении списка.
#[derive(Clone, Debug, Default)]
pub struct Children(Rc<Vec<Node>>);

impl Children {
    pub fn into_vec(mut self) -> Vec<Node> {
        match Rc::try_unwrap(std::mem::take(&mut self.0)) {
            Ok(nodes) => nodes,
            Err(nodes) => nodes.as_ref().clone(),
        }
    }
}

impl From<Vec<Node>> for Children {
    fn from(nodes: Vec<Node>) -> Self {
        Self(Rc::new(nodes))
    }
}

impl FromIterator<Node> for Children {
    fn from_iter<T: IntoIterator<Item = Node>>(iter: T) -> Self {
        Vec::from_iter(iter).into()
    }
}

impl Deref for Children {
    type Target = Vec<Node>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Children {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Rc::make_mut(&mut self.0)
    }
}

impl IntoIterator for Children {
    type Item = Node;
    type IntoIter = std::vec::IntoIter<Node>;
    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}

impl<'a> IntoIterator for &'a Children {
    type Item = &'a Node;
    type IntoIter = std::slice::Iter<'a, Node>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Children {
    type Item = &'a mut Node;
    type IntoIter = std::slice::IterMut<'a, Node>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl Drop for Children {
    fn drop(&mut self) {
        let Some(nodes) = Rc::get_mut(&mut self.0) else {
            return;
        };
        let mut pending = std::mem::take(nodes);
        while let Some(node) = pending.pop() {
            if let Node::Element(mut e) = node {
                if let Some(children) = Rc::get_mut(&mut e.children.0) {
                    pending.append(children);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
