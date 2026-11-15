//! Публикация входа той же формы: слоты переиспользуются по источнику, связи меняются на месте.
//!
//! Попутно сравнивается с прежним входом (`previous`) и ставит эпохи раскладки
//! (`epochs`): общий снимок детей с прежним означает неизменное поддерево.

use super::arena::Link;
use super::epochs::{self, Change};
use super::reuse::Reuse;
use super::source::Source;
use super::{BoxRecord, BoxTree, finish, inline, sources};
use crate::dom::{BoxId, Element, Node};

/// С чем сравниваются братья уровня обхода.
#[derive(Clone, Copy)]
enum Prev<'a> {
    /// Общий снимок с прежним входом: поддерево не изменилось.
    Clean,
    /// Прежние братья того же родителя.
    Old(&'a [Node]),
    /// Прежнего нет: всё новое.
    Fresh,
}

struct Level<'a> {
    nodes: &'a [Node],
    at: usize,
    parent: BoxId,
    owner: Option<&'a Element>,
    last: BoxId,
    prev: Prev<'a>,
    children: u64,
}

/// Прежний элемент на месте `at` среди `new` братьев: тот же индекс либо
/// сдвиг на разницу длин (одна вставка или удаление перед ним).
fn paired<'a>(old: &'a [Node], new: usize, at: usize, key: u64) -> Option<&'a Element> {
    let shifted = (at + old.len()).checked_sub(new);
    [Some(at), shifted]
        .into_iter()
        .flatten()
        .find_map(|i| match old.get(i) {
            Some(Node::Element(e)) if e.node_id == key => Some(e),
            _ => None,
        })
}

fn texts_differ(old: &[Node], new: &[Node]) -> bool {
    let texts = |nodes: &'_ [Node]| {
        nodes.iter().filter_map(|n| match n {
            Node::Text(t) => Some(t.as_str()),
            Node::Element(_) => None,
        })
    };
    !texts(old).eq(texts(new))
}

pub(super) fn mix(hash: u64, id: BoxId) -> u64 {
    let id = (u64::from(id.slot) << 32) | u64::from(id.generation);
    (hash.rotate_left(7) ^ id).wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

impl BoxTree {
    pub(super) fn publish_keyed(
        &mut self,
        nodes: &[Node],
        previous_nodes: Option<&[Node]>,
        text_keys: &[crate::dom::NodeKey],
        element_key: &dyn Fn(u64) -> Option<crate::dom::NodeKey>,
    ) {
        let sources = sources::Sources::collect(nodes, text_keys, element_key);
        let (mut previous, old, old_children) = self.previous_records();
        if self.epoch == u64::MAX {
            for &id in &old {
                self.arena.link(id, Link::Epoch(0));
            }
            self.epoch = 0;
        }
        self.epoch += 1;
        let epoch = self.epoch;
        let clock = self.tick();
        self.first = BoxId::NONE;
        self.flows.clear();
        self.by_node.clear();
        let mut changed: Vec<(BoxId, Change)> = Vec::new();
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
            let (id, fresh) = self.record(Source::Root, BoxId::NONE, 0, &mut previous);
            if fresh {
                changed.push((id, Change::Own));
            }
            id
        } else {
            BoxId::NONE
        };
        if root.is_some() {
            self.first = root;
            inline.root(root);
            self.flows.insert(finish::slice_key(nodes), root);
        }
        let prev = match previous_nodes {
            Some(p) if finish::slice_key(p) == finish::slice_key(nodes) => Prev::Clean,
            Some(p) => {
                if root.is_some() && texts_differ(p, nodes) {
                    changed.push((root, Change::Content));
                }
                Prev::Old(p)
            }
            None => Prev::Fresh,
        };
        let mut pending = vec![Level {
            nodes,
            at: 0,
            parent: root,
            owner: None,
            last: BoxId::NONE,
            prev,
            children: 0,
        }];
        while let Some(level) = pending.last_mut() {
            let Some(node) = level.nodes.get(level.at) else {
                let done = pending.pop().expect("level");
                if done.parent.is_some()
                    && !matches!(done.prev, Prev::Clean)
                    && old_children.get(&done.parent).copied().unwrap_or(0) != done.children
                {
                    changed.push((done.parent, Change::Content));
                }
                inline.leave();
                continue;
            };
            let at = level.at;
            level.at += 1;
            let Node::Element(e) = node else {
                if let Node::Text(text) = node {
                    inline.text(text);
                }
                continue;
            };
            let parent = level.parent;
            let origin = Source::of(e, parent, sources.get(e), element_key(e.node_id));
            let (id, fresh) = self.record(origin, parent, e.node_id, &mut previous);
            if !id.is_some() {
                continue;
            }
            let level = pending.last_mut().expect("level");
            level.children = mix(level.children, id);
            if level.last.is_some() {
                self.arena.link(level.last, Link::Next(id));
            } else if parent.is_some() {
                self.arena.link(parent, Link::First(id));
            } else {
                self.first = id;
            }
            level.last = id;
            let external = epochs::external_contribution(e, level.owner);
            if let Some(record) = self.arena.get_mut(id) {
                record.external = external;
            }
            let prev = match level.prev {
                Prev::Clean => Prev::Clean,
                Prev::Fresh => Prev::Fresh,
                Prev::Old(old) => match paired(old, level.nodes.len(), at, e.node_id) {
                    Some(o) if epochs::same_layout_inputs(o, e) => {
                        if o.children.identity() == e.children.identity() {
                            Prev::Clean
                        } else {
                            if texts_differ(&o.children, &e.children) {
                                changed.push((id, Change::Content));
                            }
                            Prev::Old(&o.children)
                        }
                    }
                    Some(o) => {
                        changed.push((id, Change::Own));
                        Prev::Old(&o.children)
                    }
                    None => Prev::Fresh,
                },
            };
            if fresh || matches!(prev, Prev::Fresh) {
                changed.push((id, Change::Own));
            }
            if e.node_id != 0 {
                self.by_node.entry(e.node_id).or_insert(id);
            }
            if !e.children.is_empty() {
                self.flows.insert(finish::slice_key(&e.children), id);
            }
            inline.enter(id, e);
            pending.push(Level {
                nodes: &e.children,
                at: 0,
                parent: id,
                owner: Some(e),
                last: BoxId::NONE,
                prev,
                children: 0,
            });
        }
        for (id, content) in inline.finished {
            if self.arena.inline(id, content) {
                changed.push((id, Change::Content));
            }
        }
        for id in old {
            if self.get(id).is_some_and(|r| r.epoch != epoch) {
                self.arena.remove(id);
            }
        }
        self.propagate(clock, &changed);
        // CSS text preparation belongs to publication. Frame materialization
        // rechecks only its actual inputs (not colors, fonts or viewport size).
        self.finish(nodes, &crate::style::computed::BoxStyle::default(), true);
    }

    /// Живые коробки прежней публикации: очередь переиспользования по источнику,
    /// их ключи и подпись списка детей каждой.
    fn previous_records(&self) -> (Reuse, Vec<BoxId>, std::collections::HashMap<BoxId, u64>) {
        let mut previous = Reuse::default();
        let mut old = Vec::new();
        let mut children = std::collections::HashMap::new();
        let mut pending = vec![self.first];
        while let Some(id) = pending.pop() {
            let Some(record) = self.get(id) else {
                continue;
            };
            old.push(id);
            previous.insert(record.origin.clone(), id);
            if let Some(parent) = self.get(record.parent).map(|_| record.parent) {
                let sig = children.entry(parent).or_insert(0);
                *sig = mix(*sig, id);
            }
            pending.push(record.next);
            pending.push(record.first);
        }
        (previous, old, children)
    }

    /// Слот для источника: прежний (`false`) или новый (`true`); перемещённая
    /// к другому родителю коробка считается новой.
    fn record(
        &mut self,
        origin: Source,
        parent: BoxId,
        source: u64,
        previous: &mut Reuse,
    ) -> (BoxId, bool) {
        // Очередь содержит только живые старые слоты. Каждый извлекается один
        // раз; повторный линейный поиск среди фрагментов одного узла не нужен.
        let reused = previous.take(&origin);
        let moved = reused
            .and_then(|id| self.get(id))
            .is_some_and(|r| r.parent != parent);
        let id = reused.or_else(|| {
            self.arena.insert(BoxRecord {
                parent,
                first: BoxId::NONE,
                next: BoxId::NONE,
                source,
                origin,
                inline: None,
                prepared: None,
                epoch: self.epoch,
                epochs: Default::default(),
                external: false,
                layout_slot: None,
            })
        });
        let Some(id) = id else {
            return (BoxId::NONE, false);
        };
        self.arena.link(
            id,
            Link::Reset {
                parent,
                source,
                epoch: self.epoch,
            },
        );
        (id, reused.is_none() || moved)
    }
}
