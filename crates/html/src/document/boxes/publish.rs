//! Публикация входа той же формы: слоты переиспользуются по источнику, связи меняются на месте.

use super::arena::Link;
use super::reuse::Reuse;
use super::source::Source;
use super::{BoxRecord, BoxTree, finish, inline, sources};
use crate::dom::{BoxId, Node};

impl BoxTree {
    pub(super) fn publish_keyed(
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
}
