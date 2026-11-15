//! Normalize IFC text across inline boundaries after materialization, before GPUI build.

mod mixed;
#[cfg(test)]
mod mixed_adapter_tests;
#[cfg(test)]
mod mixed_tests;
#[cfg(test)]
mod raw_text_tests;
#[cfg(test)]
mod sharing_tests;
#[cfg(test)]
mod tests;

use super::{
    BoxTree,
    inline::{InlineContent, Item, ItemKind},
};
use crate::dom::{BoxId, Node};
use crate::style::computed::{BoxStyle, Computed};
use crate::text::inline::normalization::{Inputs, Normalizer};
use std::rc::Rc;

#[derive(Debug)]
pub(super) struct Prepared {
    pub(super) ready: bool,
    source: Rc<InlineContent>,
    inputs: Inputs,
    dependencies: Rc<[(BoxId, Inputs)]>,
    pub(super) text: Rc<InlineContent>,
}

/// A prepared flow with inline boxes: one normalized run per source text item
/// (empty runs kept), so the paragraph adapter can take each text node's run in
/// document order. `inputs` are those of the descendant boxes the runs read.
pub(crate) struct MixedText {
    pub(crate) owner: BoxId,
    pub(crate) source: Rc<InlineContent>,
    pub(crate) text: Rc<InlineContent>,
    pub(crate) inputs: Rc<[(BoxId, Inputs)]>,
}

/// Стили потоков, которые публикация обошла: только их подготовка может устареть.
pub(super) type Visited = mixed::Styles;

impl BoxTree {
    /// Addresses only select records of this published adapter; no pointer is dereferenced.
    pub(crate) fn finish(&mut self, nodes: &[Node], root: &BoxStyle, changed: bool) {
        if !changed
            && self
                .finished
                .as_ref()
                .is_some_and(|(epoch, old)| *epoch == self.epoch && old.matches(root))
        {
            return;
        }
        let same_root = self
            .finished
            .as_ref()
            .is_some_and(|(_, old)| old.matches(root));
        self.finished = Some((self.epoch, Inputs::of(root)));
        let visited = std::mem::take(&mut self.visited);
        if !changed && same_root {
            // Новая публикация при тех же вычисленных стилях: устареть могла
            // только подготовка обойдённых ею потоков.
            self.prepare_visited(nodes, root, &visited);
            return;
        }
        let mut styles = std::collections::HashMap::new();
        let mut pending = vec![nodes.iter()];
        while let Some(siblings) = pending.last_mut() {
            match siblings.next() {
                Some(Node::Element(e)) => {
                    if let Some(id) = self.flow_of(&e.children) {
                        styles.insert(
                            id,
                            (e.style.used().style, e.style.slot().published().is_some()),
                        );
                    }
                    pending.push(e.children.iter());
                }
                Some(Node::Text(_)) => {}
                None => {
                    pending.pop();
                }
            }
        }
        if let Some(id) = self.flow_of(nodes) {
            self.prepare_with(id, root, &styles);
        }
        for (&id, (style, _)) in &styles {
            self.prepare_with(id, style, &styles);
        }
    }

    /// Подготовить корневой поток (стилем `root`) и обойдённые потоки.
    pub(super) fn prepare_visited(&mut self, nodes: &[Node], root: &BoxStyle, styles: &Visited) {
        if let Some(id) = self.flow_of(nodes) {
            self.prepare_with(id, root, styles);
        }
        for (&id, (style, _)) in styles {
            self.prepare_with(id, style, styles);
        }
    }

    /// A synthetic or changed layout parent must use its own normalization inputs.
    pub(crate) fn normalized(&self, nodes: &[Node], style: &Computed) -> Option<Rc<InlineContent>> {
        let text = self.normalized_inline(self.flow_of(nodes)?, style)?;
        text.items
            .iter()
            .all(|item| matches!(item.kind, ItemKind::Text { .. }))
            .then_some(text)
    }

    /// The prepared runs of a flow with inline boxes (`normalized` covers flat text).
    pub(crate) fn mixed_text(&self, nodes: &[Node], style: &Computed) -> Option<MixedText> {
        let owner = self.flow_of(nodes)?;
        let prepared = self.get(owner)?.prepared.as_ref()?;
        let text = self.normalized_inline(owner, style)?;
        text.items
            .iter()
            .any(|item| !matches!(item.kind, ItemKind::Text { .. }))
            .then(|| MixedText {
                owner,
                source: prepared.source.clone(),
                text,
                inputs: prepared.dependencies.clone(),
            })
    }

    /// Stable T3 input: offsets address this UTF-8 buffer; BoxIds identify enclosing boxes.
    pub(crate) fn normalized_inline(
        &self,
        id: BoxId,
        style: &Computed,
    ) -> Option<Rc<InlineContent>> {
        let record = self.get(id)?;
        let prepared = record.prepared.as_ref()?;
        (prepared.ready && prepared.inputs.matches(style)).then(|| prepared.text.clone())
    }
}

impl BoxTree {
    #[cfg(test)]
    pub(super) fn prepare(&mut self, id: BoxId, style: &Computed) {
        self.prepare_with(id, style, &Default::default());
    }

    fn prepare_with(&mut self, id: BoxId, style: &Computed, styles: &mixed::Styles) {
        let Some(record) = self.get(id) else { return };
        let Some(source) = record.inline.as_ref() else {
            self.arena.prepared(id, None);
            return;
        };
        // Opaque leaves need their separate atom/ruby lowering before normalization.
        if !source.items.iter().all(|item| {
            matches!(
                item.kind,
                ItemKind::Text { .. } | ItemKind::Open | ItemKind::Close
            )
        }) {
            self.arena.prepared(id, None);
            return;
        }
        if record.prepared.as_ref().is_some_and(|p| {
            Rc::ptr_eq(&p.source, source)
                && p.inputs.matches(style)
                && mixed::matches(&p.dependencies, styles)
        }) {
            self.arena.prepared_ready(id, true);
            return;
        }
        let mut normalizer = Normalizer::default();
        let mut text = InlineContent::default();
        let mut dependencies = Vec::new();
        for item in &source.items {
            let ItemKind::Text { start, end } = item.kind else {
                text.items.push(item.clone());
                continue;
            };
            let Some(input) = mixed::input(id, item.box_id, style, styles, &mut dependencies)
            else {
                // Keep the previous immutable owner while computed descendants
                // are pending, but never expose it as the current IFC input.
                self.arena.prepared_ready(id, false);
                return;
            };
            let run = normalizer.text(&source.text[start..end], input);
            let start = text.text.len();
            text.text.push_str(&run);
            text.items.push(Item {
                box_id: item.box_id,
                kind: ItemKind::Text {
                    start,
                    end: text.text.len(),
                },
            });
        }
        let prepared = Prepared {
            ready: true,
            source: source.clone(),
            inputs: Inputs::of(style),
            dependencies: dependencies.into(),
            text: if text == **source {
                source.clone()
            } else {
                text.text.shrink_to_fit();
                text.items.shrink_to_fit();
                Rc::new(text)
            },
        };
        self.arena.prepared(id, Some(prepared));
    }
}

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for Prepared {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        self.source.heap_size(ops)
            + self.inputs.flow_heap_size(ops)
            + self.text.heap_size(ops)
            + ops.buffer(self.dependencies.as_ptr(), self.dependencies.len())
            + self
                .dependencies
                .iter()
                .map(|(_, input)| input.flow_heap_size(ops))
                .sum::<usize>()
    }
}
