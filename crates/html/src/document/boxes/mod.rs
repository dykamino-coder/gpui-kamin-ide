//! Дерево коробок документа: стабильные слоты и изменяемые связи между ними.
//!
//! Входное Vec<Node> пока служит адаптером подготовки. Арена не хранит его снимков:
//! публикация меняет существующие записи, а удаление увеличивает поколение слота.

mod adapter;
mod arena;
#[cfg(test)]
mod box_of_tests;
mod finish;
pub(crate) use finish::MixedText;
#[cfg(feature = "heap-report")]
mod heap;
pub(crate) mod inline;
#[cfg(test)]
mod key_tests;
mod reuse;
mod source;
#[cfg(test)]
mod source_tests;
mod sources;
#[cfg(test)]
mod tests;
pub(crate) mod used;

use crate::dom::{BoxId, Node};
use arena::{Arena, Link};
use reuse::Reuse;
use source::Source;
use std::rc::Rc;

pub(crate) struct BoxRecord {
    pub(crate) parent: BoxId,
    pub(crate) first: BoxId,
    pub(crate) next: BoxId,
    source: u64,
    origin: Source,
    pub(crate) inline: Option<Rc<inline::InlineContent>>,
    prepared: Option<Box<finish::Prepared>>,
    epoch: u64,
}

#[derive(Default)]
pub(crate) struct BoxTree {
    arena: Arena,
    flows: std::collections::HashMap<(usize, usize), BoxId>,
    /// The first box of each keyed source element (`Element::node_id`):
    /// layout finds its box in O(1); fragment copies share it.
    by_node: std::collections::HashMap<u64, BoxId>,
    first: BoxId,
    epoch: u64,
    finished: Option<(u64, crate::text::inline::normalization::Inputs)>,
    /// Used styles derived from computed ones, shared by the boxes of one style.
    pub(crate) used: used::UsedStyles,
}

impl BoxTree {
    pub(crate) fn is_empty(&self) -> bool {
        !self.first.is_some()
    }

    /// The one mutable entry to a document's box tree. Used styles in it are
    /// shared values (`used`): no caller reaches a `Computed` through it.
    pub(crate) fn edit(tree: &std::cell::RefCell<Self>) -> std::cell::RefMut<'_, Self> {
        tree.borrow_mut()
    }

    /// The box of the source element with key `node_id` (its first fragment).
    pub(crate) fn box_of(&self, node_id: u64) -> Option<BoxId> {
        self.by_node.get(&node_id).copied()
    }

    pub(crate) fn get(&self, id: BoxId) -> Option<&BoxRecord> {
        self.arena.get(id)
    }

    /// Живые коробки без обхода адаптерного дерева Element/Children.
    /// Порядок слотов не задаёт порядок документа: его задают first/next.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (BoxId, &BoxRecord)> {
        self.arena.iter()
    }

    /// Опубликовать принятый вход той же формы.
    /// Источник задаётся NodeKey DOM, а синтетическая роль — отдельно от него.
    #[cfg(test)]
    fn publish(&mut self, nodes: &[Node]) {
        self.publish_sources(nodes, &[]);
    }

    pub(crate) fn publish_document(
        &mut self,
        nodes: &[Node],
        values: &crate::values::DocumentValues,
    ) {
        self.publish_keyed(nodes, &values.traversal.text_keys(), &|id| {
            Some(crate::dom::NodeKey::from_packed(id))
        });
    }

    #[cfg(test)]
    fn publish_sources(&mut self, nodes: &[Node], text_keys: &[crate::dom::NodeKey]) {
        self.publish_keyed(nodes, text_keys, &|id| {
            Some(crate::dom::NodeKey::from_packed(id))
        });
    }

    fn publish_keyed(
        &mut self,
        nodes: &[Node],
        text_keys: &[crate::dom::NodeKey],
        element_key: &dyn Fn(u64) -> Option<crate::dom::NodeKey>,
    ) {
        let sources = sources::Sources::collect(nodes, text_keys, element_key);
        let mut previous = Reuse::default();
        let mut old = Vec::new();
        let mut pending = vec![self.first];
        while let Some(id) = pending.pop() {
            let Some(record) = self.get(id) else {
                continue;
            };
            old.push(id);
            previous.insert(record.origin.clone(), id);
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
        self.flows.clear();
        self.by_node.clear();
        let mut inline = inline::Collection::default();
        // Фрагмент может состоять из текста или соседних inline без Element-
        // родителя. Его IFC принадлежит документной коробке, а не первому span.
        let root = if nodes.iter().any(|node| match node {
            Node::Text(_) => true,
            Node::Element(e) => {
                crate::layout::box_kind::inline_level_box(e)
                    || e.style.used().reset_box_layout.display
                        == Some(crate::style::computed::Display::Contents)
            }
        }) {
            self.record(Source::Root, BoxId::NONE, 0, &mut previous)
                .unwrap_or(BoxId::NONE)
        } else {
            BoxId::NONE
        };
        if root.is_some() {
            self.first = root;
            inline.root(root);
            self.flows.insert(finish::slice_key(nodes), root);
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
            let origin = Source::of(e, *parent, sources.get(e), element_key(e.node_id));
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
            if e.node_id != 0 {
                self.by_node.entry(e.node_id).or_insert(id);
            }
            if !e.children.is_empty() {
                self.flows.insert(finish::slice_key(&e.children), id);
            }
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
        // CSS text preparation belongs to publication. Frame materialization
        // rechecks only its actual inputs (not colors, fonts or viewport size).
        self.finish(nodes, &crate::style::computed::BoxStyle::default(), true);
    }

    fn record(
        &mut self,
        origin: Source,
        parent: BoxId,
        source: u64,
        previous: &mut Reuse,
    ) -> Option<BoxId> {
        // Очередь содержит только живые старые слоты. Каждый извлекается один
        // раз; повторный линейный поиск среди фрагментов одного узла не нужен.
        let id = previous.take(&origin);
        let id = id.or_else(|| {
            self.arena.insert(BoxRecord {
                parent,
                first: BoxId::NONE,
                next: BoxId::NONE,
                source,
                origin,
                inline: None,
                prepared: None,
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
    pub(crate) fn replace(&mut self, nodes: &[Node], values: &crate::values::DocumentValues) {
        let ids: Vec<_> = self.iter().map(|(id, _)| id).collect();
        for id in ids {
            self.arena.remove(id);
        }
        self.first = BoxId::NONE;
        self.publish_document(nodes, values);
    }
}
