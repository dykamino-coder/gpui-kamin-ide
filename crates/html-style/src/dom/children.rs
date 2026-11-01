//! Общие дети элемента: копия коробки не копирует поддерево и не углубляет drop.

use super::Node;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// Дети элемента с копированием при изменении списка.
#[derive(Clone, Debug, Default)]
pub struct Children(Rc<Vec<Node>>);

impl Children {
    /// Идентичность неизменяемого снимка: изменение списка отделяет его через COW.
    pub fn identity(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }

    /// Метка снимка без владения узлами: пока она жива, адрес `identity` не
    /// достаётся другому списку, а поддерево освобождает последний владелец.
    pub fn mark(&self) -> std::rc::Weak<Vec<Node>> {
        Rc::downgrade(&self.0)
    }

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

/// Единственный владелец списка; слабые метки снимков отделяются от узлов
/// без их копирования, чтобы drop оставался итеративным.
fn sole(nodes: &mut Rc<Vec<Node>>) -> Option<Vec<Node>> {
    if let Some(nodes) = Rc::get_mut(nodes) {
        return Some(std::mem::take(nodes));
    }
    (Rc::strong_count(nodes) == 1)
        .then(|| Rc::try_unwrap(std::mem::take(nodes)).ok())
        .flatten()
}

impl Drop for Children {
    fn drop(&mut self) {
        let Some(mut pending) = sole(&mut self.0) else {
            return;
        };
        while let Some(node) = pending.pop() {
            if let Node::Element(mut e) = node {
                if let Some(mut children) = sole(&mut e.children.0) {
                    pending.append(&mut children);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
