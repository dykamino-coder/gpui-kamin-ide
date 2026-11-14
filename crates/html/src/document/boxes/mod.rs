//! Дерево коробок документа: стабильные слоты и изменяемые связи между ними.
//!
//! Входное Vec<Node> пока служит адаптером подготовки. Арена не хранит его снимков:
//! публикация меняет существующие записи, а удаление увеличивает поколение слота.

mod arena;
#[cfg(test)]
mod tests;

use crate::dom::{BoxId, Node};
use arena::Arena;
use std::collections::{HashMap, VecDeque};

pub(crate) struct BoxRecord {
    pub(crate) parent: BoxId,
    pub(crate) first: BoxId,
    pub(crate) next: BoxId,
    source: u64,
    epoch: u64,
}

#[derive(Default)]
pub(crate) struct BoxTree {
    arena: Arena<BoxRecord>,
    first: BoxId,
    epoch: u64,
}

impl BoxTree {
    pub(crate) fn is_empty(&self) -> bool {
        !self.first.is_some()
    }

    pub(crate) fn get(&self, id: BoxId) -> Option<&BoxRecord> {
        self.arena.get(id)
    }

    /// Опубликовать принятый вход той же формы.
    /// Полная сборка той же формы пока связывается по прежним source id;
    /// это адаптер ревизии, а не постоянный NodeKey исходного DOM.
    pub(crate) fn publish(&mut self, nodes: &[Node]) {
        let mut previous: HashMap<u64, VecDeque<BoxId>> = HashMap::new();
        let mut old = Vec::new();
        let mut pending = vec![self.first];
        while let Some(id) = pending.pop() {
            let Some(record) = self.get(id) else {
                continue;
            };
            old.push(id);
            previous.entry(record.source).or_default().push_back(id);
            pending.push(record.next);
            pending.push(record.first);
        }
        if self.epoch == u64::MAX {
            for &id in &old {
                if let Some(record) = self.arena.get_mut(id) {
                    record.epoch = 0;
                }
            }
            self.epoch = 0;
        }
        self.epoch += 1;
        let epoch = self.epoch;
        self.first = BoxId::NONE;
        let mut pending = vec![(nodes.iter(), BoxId::NONE, BoxId::NONE)];
        while let Some((siblings, parent, last)) = pending.last_mut() {
            let Some(node) = siblings.next() else {
                pending.pop();
                continue;
            };
            let Node::Element(e) = node else { continue };
            let fits = |id| {
                self.arena
                    .get(id)
                    .is_some_and(|r| r.source == e.node_id && r.epoch != epoch)
            };
            let id = previous.get_mut(&e.node_id).and_then(|ids| {
                ids.iter()
                    .position(|&id| fits(id))
                    .and_then(|at| ids.remove(at))
            });
            let id = id.or_else(|| {
                self.arena.insert(BoxRecord {
                    parent: *parent,
                    first: BoxId::NONE,
                    next: BoxId::NONE,
                    source: e.node_id,
                    epoch,
                })
            });
            let Some(id) = id else { continue };
            let Some(record) = self.arena.get_mut(id) else {
                continue;
            };
            record.parent = *parent;
            record.first = BoxId::NONE;
            record.next = BoxId::NONE;
            record.epoch = epoch;
            if last.is_some() {
                if let Some(record) = self.arena.get_mut(*last) {
                    record.next = id;
                }
            } else if parent.is_some() {
                if let Some(record) = self.arena.get_mut(*parent) {
                    record.first = id;
                }
            } else {
                self.first = id;
            }
            *last = id;
            pending.push((e.children.iter(), id, BoxId::NONE));
        }
        for id in old {
            if self.get(id).is_some_and(|r| r.epoch != epoch) {
                self.arena.remove(id);
            }
        }
    }

    /// Полная смена формы до появления постоянных ключей DOM: старые ключи
    /// коробок инвалидируются, а слоты получают новые поколения.
    pub(crate) fn replace(&mut self, nodes: &[Node]) {
        let ids: Vec<_> = self.arena.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.arena.remove(id);
        }
        self.first = BoxId::NONE;
        self.publish(nodes);
    }
}
