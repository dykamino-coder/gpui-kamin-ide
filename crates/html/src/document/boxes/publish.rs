//! Publication of an input: slots are reused by source, links change in place.
//!
//! With a previous input (`previous`) the publication walks only what changed: an element
//! whose children snapshot is shared with the previous one and whose layout inputs are equal keeps
//! its box subtree as it is (links, IFC, prepared text), unless it
//! gives its text to an ancestor's IFC. The layout epochs are set on the way (`epochs`).

use super::claims::Claims;
use super::epochs;
use super::level::{Level, Prev, paired, texts_differ};
use super::source::Source;
use super::{BoxTree, finish, inline};
use crate::document::delta::{BoxDelta, Damage};
use crate::dom::{BoxId, Element, Node};

/// What the publication knows about the input besides the nodes themselves.
pub(super) struct Keys<'a> {
    pub(super) text: &'a [crate::dom::NodeKey],
    pub(super) element: &'a dyn Fn(u64) -> Option<crate::dom::NodeKey>,
}

impl BoxTree {
    pub(super) fn publish_keyed(
        &mut self,
        nodes: &[Node],
        previous: Option<&[Node]>,
        keys: &Keys,
    ) -> BoxDelta {
        self.epoch = self.epoch.wrapping_add(1);
        let clock = self.tick();
        let mut changed = BoxDelta::default();
        let mut claims = Claims::new(self);
        let mut inline = inline::Collection::default();
        let mut visited = finish::Visited::default();
        let prev = match previous {
            // The document level is compared node by node: a slice carries no
            // identity of its contents (the levels below compare their
            // children snapshots).
            Some(p) => Prev::Old(p),
            None => Prev::Fresh,
        };
        // A fragment may consist of text or adjacent inlines without an Element
        // parent. Its IFC belongs to the document box, not to the first span.
        let needs_root = nodes.iter().any(|node| match node {
            Node::Text(_) => true,
            Node::Element(e) => {
                crate::layout::box_kind::inline_level_box(e)
                    || e.style.used().reset_box_layout.display
                        == Some(crate::style::computed::Display::Contents)
            }
        });
        let old_first = self.first;
        let old_root = self
            .get(old_first)
            .is_some_and(|r| r.origin == Source::Root);
        let (root, first_old) = if needs_root {
            let reused = old_root.then_some(old_first);
            let root = self.record(Source::Root, BoxId::NONE, 0, reused, &mut changed);
            if root.is_some() {
                inline.root(root);
                self.set_root_flow(nodes, root);
                if let Prev::Old(p) = prev
                    && texts_differ(p, nodes)
                {
                    changed.mark(root, Damage::CONTENT);
                }
            }
            let first_old =
                reused.map_or(old_first, |r| self.get(r).map_or(BoxId::NONE, |r| r.first));
            self.arena.unlink_first(root);
            if reused.is_none() && old_root {
                claims.remove(old_first);
            }
            self.first = root;
            (root, first_old)
        } else {
            if old_root {
                let below = self.get(old_first).map_or(BoxId::NONE, |r| r.first);
                self.arena.unlink_first(old_first);
                claims.remove(old_first);
                (BoxId::NONE, below)
            } else {
                (BoxId::NONE, old_first)
            }
        };
        if !root.is_some() {
            self.first = BoxId::NONE;
        }
        super::reservations::reserve(self, &mut claims, nodes, prev, keys, root, first_old);
        let mut pending = vec![Level::new(nodes, root, None, prev, true, first_old)];
        while let Some(level) = pending.last_mut() {
            let Some(node) = level.next_node() else {
                let done = pending.pop().expect("level");
                let parent = done.parent;
                let reshaped = done.finish(self, &mut claims);
                if reshaped && parent.is_some() {
                    changed.mark(parent, Damage::CHILDREN);
                }
                inline.leave();
                continue;
            };
            let (at, len) = (level.at - 1, level.nodes.len());
            let Node::Element(e) = node else {
                if let Node::Text(text) = node {
                    inline.text(text);
                }
                continue;
            };
            let parent = level.parent;
            let origin = Source::of(e, parent, self.extent(e, keys), (keys.element)(e.node_id));
            let reused = level
                .take(self, &mut claims, &origin)
                .or_else(|| claims.take(self, &origin, e.node_id));
            let id = self.record(origin, parent, e.node_id, reused, &mut changed);
            if !id.is_some() {
                continue;
            }
            claims.accept(id);
            let level = pending.last_mut().expect("level");
            level.append(self, id);
            if let Some(record) = self.arena.record_mut(id) {
                record.external = epochs::external_contribution(e, level.owner);
                record.distributes = epochs::distributes(e);
            }
            let mut inherited = level.inherited;
            let prev = match level.prev {
                Prev::Clean => Prev::Clean,
                Prev::Fresh => Prev::Fresh,
                Prev::Old(old) => match paired(old, len, at, e.node_id) {
                    Some(o) if epochs::same_layout_inputs(o, e, inherited) => {
                        let same = if inherited {
                            crate::dom::same_cascaded_values(o, e)
                        } else {
                            crate::dom::same_box(o, e)
                        };
                        if !same {
                            changed.mark(id, Damage::PAINT);
                        }
                        if o.children.identity() == e.children.identity() {
                            Prev::Clean
                        } else {
                            if texts_differ(&o.children, &e.children) {
                                changed.mark(id, Damage::CONTENT);
                            }
                            Prev::Old(&o.children)
                        }
                    }
                    Some(o) => {
                        inherited = false;
                        changed.mark(id, Damage::LAYOUT);
                        Prev::Old(&o.children)
                    }
                    None => Prev::Fresh,
                },
            };
            if matches!(prev, Prev::Fresh) {
                changed.mark(id, Damage::LAYOUT);
            }
            if e.node_id != 0 {
                self.by_node.insert_box(e.node_id, id);
            }
            let kept = reused == Some(id) && matches!(prev, Prev::Clean) && !inline.feeds_owner(e);
            if kept {
                claims.keep(id);
                // An unchanged subtree: links, IFC and preparation stay; the ancestor
                // gets only the line item of this box.
                inline.skip(id, e);
                continue;
            }
            let old_children = reused
                .and_then(|r| self.get(r))
                .map_or(BoxId::NONE, |r| r.first);
            self.arena.unlink_first(id);
            self.set_flow(&e.children, id);
            if !e.children.is_empty() {
                let style = e.style.used().style;
                visited.insert(id, (style, e.style.slot().published().is_some()));
            }
            inline.enter(id, e);
            pending.push(Level::new(
                &e.children,
                id,
                Some(e),
                prev,
                inherited,
                old_children,
            ));
        }
        claims.finish(self, &mut changed);
        let finished = inline.seal(|id| self.get(id).and_then(|r| r.inline.as_ref()));
        for (id, content) in finished {
            if self.arena.inline(id, content) {
                changed.mark(id, Damage::CONTENT);
            }
        }
        self.propagate(clock, &changed);
        // CSS text preparation belongs to publication, only for the flows it
        // walked. Frame materialization rechecks only its actual inputs (not
        // colors, fonts or viewport size).
        self.prepare_visited(
            nodes,
            &crate::style::computed::BoxStyle::default(),
            &visited,
        );
        self.visited = visited;
        if previous.is_none() {
            self.arena.fit();
            self.by_node.fit();
        }
        changed
    }

    /// The bounds of an anonymous fragment: the first and last source key inside
    /// (an authored element gives its own key without going inside).
    pub(super) fn extent(&self, e: &Element, keys: &Keys) -> super::sources::Extent {
        if Source::authored(e) && (keys.element)(e.node_id).is_some() {
            return Default::default();
        }
        super::sources::Extent::of(e, keys.text, keys.element)
    }
}
