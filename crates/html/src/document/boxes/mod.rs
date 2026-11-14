//! Дерево коробок документа: стабильные слоты и изменяемые связи между ними.
//!
//! Входное Vec<Node> пока служит адаптером подготовки. Арена не хранит его снимков:
//! публикация меняет существующие записи, а удаление увеличивает поколение слота.

mod arena;
mod inline;
mod source;
#[cfg(test)]
mod tests;

use crate::dom::{BoxId, Node};
use arena::{Arena, Link};
use source::Source;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

pub(crate) struct BoxRecord {
    pub(crate) parent: BoxId,
    pub(crate) first: BoxId,
    pub(crate) next: BoxId,
    source: u64,
    origin: Source,
    pub(crate) inline: Option<Rc<inline::InlineContent>>,
    epoch: u64,
}

#[derive(Default)]
pub(crate) struct BoxTree {
    arena: Arena,
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
    /// Источник задаётся NodeKey DOM, а синтетическая роль — отдельно от него.
    pub(crate) fn publish(&mut self, nodes: &[Node]) {
        let mut previous: HashMap<Source, VecDeque<BoxId>> = HashMap::new();
        let mut old = Vec::new();
        let mut pending = vec![self.first];
        while let Some(id) = pending.pop() {
            let Some(record) = self.get(id) else {
                continue;
            };
            old.push(id);
            previous
                .entry(record.origin.clone())
                .or_default()
                .push_back(id);
            pending.push(record.next);
            pending.push(record.first);
        }
        if self.epoch == u64::MAX {
            for &id in &old {
                self.arena.link(id, Link::Epoch(0));
            }
            self.epoch = 0;
        }
        self.epoch += 1;
        let epoch = self.epoch;
        self.first = BoxId::NONE;
        let mut inline = inline::Collection::default();
        let mut pending = vec![(nodes.iter(), BoxId::NONE, BoxId::NONE)];
        while let Some((siblings, parent, last)) = pending.last_mut() {
            let Some(node) = siblings.next() else {
                pending.pop();
                inline.leave();
                continue;
            };
            let Node::Element(e) = node else {
                if let Node::Text(text) = node {
                    inline.text(text);
                }
                continue;
            };
            let origin = Source::of(e);
            let fits = |id| {
                self.arena
                    .get(id)
                    .is_some_and(|r| r.origin == origin && r.epoch != epoch)
            };
            let id = previous.get_mut(&origin).and_then(|ids| {
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
                    origin,
                    inline: None,
                    epoch,
                })
            });
            let Some(id) = id else { continue };
            self.arena.link(
                id,
                Link::Reset {
                    parent: *parent,
                    source: e.node_id,
                    epoch,
                },
            );
            if last.is_some() {
                self.arena.link(*last, Link::Next(id));
            } else if parent.is_some() {
                self.arena.link(*parent, Link::First(id));
            } else {
                self.first = id;
            }
            *last = id;
            inline.enter(id, e);
            pending.push((e.children.iter(), id, BoxId::NONE));
        }
        for (id, content) in inline.finished {
            self.arena.inline(id, content);
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
