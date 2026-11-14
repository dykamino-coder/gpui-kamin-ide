//! Дерево коробок документа: стабильные слоты и изменяемые связи между ними.
//!
//! Входное Vec<Node> пока служит адаптером подготовки. Арена не хранит его снимков:
//! публикация меняет существующие записи, а удаление увеличивает поколение слота.

mod arena;
mod inline;
#[cfg(test)]
mod key_tests;
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
        // Фрагмент может состоять из текста или соседних inline без Element-
        // родителя. Его IFC принадлежит документной коробке, а не первому span.
        let root = if nodes.iter().any(|node| match node {
            Node::Text(_) => true,
            Node::Element(e) => crate::layout::box_kind::inline_level_box(e),
        }) {
            self.record(Source::Root, BoxId::NONE, 0, &mut previous)
                .unwrap_or(BoxId::NONE)
        } else {
            BoxId::NONE
        };
        if root.is_some() {
            self.first = root;
            inline.root(root);
        }
        let mut pending = vec![(nodes.iter(), root, BoxId::NONE)];
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
            let id = self.record(origin, *parent, e.node_id, &mut previous);
            let Some(id) = id else { continue };
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

    fn record(
        &mut self,
        origin: Source,
        parent: BoxId,
        source: u64,
        previous: &mut HashMap<Source, VecDeque<BoxId>>,
    ) -> Option<BoxId> {
        // Очередь содержит только живые старые слоты. Каждый извлекается один
        // раз; повторный линейный поиск среди фрагментов одного узла не нужен.
        let id = previous.get_mut(&origin).and_then(VecDeque::pop_front);
        let id = id.or_else(|| {
            self.arena.insert(BoxRecord {
                parent,
                first: BoxId::NONE,
                next: BoxId::NONE,
                source,
                origin,
                inline: None,
                epoch: self.epoch,
            })
        })?;
        self.arena.link(
            id,
            Link::Reset {
                parent,
                source,
                epoch: self.epoch,
            },
        );
        Some(id)
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
