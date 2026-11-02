//! Общие дети элемента: копия коробки не копирует поддерево и не углубляет drop.

use super::Node;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

/// Дети элемента с копированием при изменении списка.
#[derive(Clone, Debug, Default)]
pub struct Children(Rc<Snapshot>);

/// Метка идентичности снимка без владения узлами и intrinsic-результатом.
#[derive(Clone, Debug)]
pub struct ChildrenMark(std::rc::Weak<Snapshot>);

impl ChildrenMark {
    /// Последний владелец снимка ещё не освободил его содержимое.
    pub fn is_alive(&self) -> bool {
        self.0.strong_count() != 0
    }
}

#[derive(Debug, Default)]
struct Snapshot {
    nodes: Vec<Node>,
    intrinsic: super::intrinsic::IntrinsicCache,
}

impl Clone for Snapshot {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            intrinsic: Default::default(),
        }
    }
}

impl Children {
    /// Слот узла переживает кадры, пока неизменны стиль и входы intrinsic-меры.
    pub fn intrinsic_sizes(&self, input: super::IntrinsicInput) -> super::IntrinsicSizes {
        self.0.intrinsic.sizes(input)
    }

    /// Изменение computed-стиля самого узла или его потомка.
    pub fn dirty_intrinsic_sizes(&self) {
        self.0.intrinsic.dirty();
    }

    /// Идентичность неизменяемого снимка: изменение списка отделяет его через COW.
    pub fn identity(&self) -> usize {
        Rc::as_ptr(&self.0) as usize
    }

    /// Метка снимка без владения узлами: пока она жива, адрес `identity` не
    /// достаётся другому списку, а поддерево освобождает последний владелец.
    pub fn mark(&self) -> ChildrenMark {
        ChildrenMark(Rc::downgrade(&self.0))
    }

    pub fn into_vec(mut self) -> Vec<Node> {
        match Rc::try_unwrap(std::mem::take(&mut self.0)) {
            Ok(snapshot) => snapshot.nodes,
            Err(snapshot) => snapshot.nodes.clone(),
        }
    }
}

impl From<Vec<Node>> for Children {
    fn from(mut nodes: Vec<Node>) -> Self {
        // Опубликованный COW-снимок удерживается вместе с intrinsic-кэшем.
        // Запас ёмкости построителя больше не нужен: при следующей правке
        // Vec снова растёт обычным способом, а неизменённые копии делят снимок.
        nodes.shrink_to_fit();
        Self(Rc::new(Snapshot {
            nodes,
            intrinsic: Default::default(),
        }))
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
        &self.0.nodes
    }
}

impl DerefMut for Children {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Правка текста, стиля потомка или списка отделяет только изменяемую
        // ветвь. Копии старого снимка сохраняют свой корректный результат.
        let snapshot = Rc::make_mut(&mut self.0);
        snapshot.intrinsic.dirty();
        &mut snapshot.nodes
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
fn sole(snapshot: &mut Rc<Snapshot>) -> Option<Vec<Node>> {
    if let Some(snapshot) = Rc::get_mut(snapshot) {
        return Some(std::mem::take(&mut snapshot.nodes));
    }
    (Rc::strong_count(snapshot) == 1)
        .then(|| {
            Rc::try_unwrap(std::mem::take(snapshot))
                .ok()
                .map(|s| s.nodes)
        })
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
