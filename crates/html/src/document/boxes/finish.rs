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
    inline::{self, InlineContent, Item, ItemKind},
};
use crate::dom::{BoxId, Node};
use crate::style::computed::{BoxStyle, Computed};
use crate::text::inline::normalization::{Inputs, Normalizer};
use std::rc::Rc;

/// The CSS Text preparation of a flow. It goes with its source: a changed
/// `BoxRecord::inline` drops it (`Arena::inline`).
#[derive(Debug)]
pub(super) struct Prepared {
    pub(super) ready: bool,
    inputs: Inputs,
    /// The inputs of descendant boxes the runs read; `None` when there are none.
    dependencies: Option<Rc<[(BoxId, Inputs)]>>,
    /// The normalized runs; `None` when they equal the source.
    text: Option<InlineContent>,
}

impl Prepared {
    /// The normalized runs when they differ from the source.
    pub(super) fn normalized(&self) -> Option<&InlineContent> {
        self.text.as_ref()
    }
}

/// A prepared flow with inline boxes: one normalized run per source text item
/// (empty runs kept), so the paragraph adapter can take each text node's run in
/// document order. `inputs` are those of the descendant boxes the runs read.
pub(crate) struct MixedText {
    pub(crate) owner: BoxId,
    pub(crate) source: InlineContent,
    pub(crate) text: InlineContent,
    pub(crate) inputs: Option<Rc<[(BoxId, Inputs)]>>,
}

/// Styles of the flows the publication walked: only their preparation can be stale.
pub(super) type Visited = mixed::Styles;

impl BoxTree {
    /// Addresses only select records of this published adapter; no pointer is dereferenced.
    pub(crate) fn finish(&mut self, nodes: &[Node], root: &BoxStyle, changed: bool) {
        if !changed
            && self.finished.as_ref().is_some_and(|(epoch, old)| {
                *epoch == self.epoch && old.matches(root, root.reset_text_box.ruby_suppress_breaks)
            })
        {
            return;
        }
        let same_root = self
            .finished
            .as_ref()
            .is_some_and(|(_, old)| old.matches(root, root.reset_text_box.ruby_suppress_breaks));
        self.finished = Some((
            self.epoch,
            Inputs::of(root, root.reset_text_box.ruby_suppress_breaks),
        ));
        let visited = std::mem::take(&mut self.visited);
        if !changed && same_root {
            // A new publication under the same computed styles: only the preparation
            // of the flows it walked can be stale.
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

    /// Prepare the root flow (with style `root`) and the walked flows.
    pub(super) fn prepare_visited(&mut self, nodes: &[Node], root: &BoxStyle, styles: &Visited) {
        if let Some(id) = self.flow_of(nodes) {
            self.prepare_with(id, root, styles);
        }
        for (&id, (style, _)) in styles {
            self.prepare_with(id, style, styles);
        }
    }

    /// A synthetic or changed layout parent must use its own normalization inputs.
    pub(crate) fn normalized(&self, nodes: &[Node], style: &Computed) -> Option<InlineContent> {
        let text = self.normalized_inline(self.flow_of(nodes)?, style)?;
        text.items()
            .iter()
            .all(|item| matches!(item.kind, ItemKind::Text { .. }))
            .then_some(text)
    }

    /// The prepared runs of a flow with inline boxes (`normalized` covers flat text).
    pub(crate) fn mixed_text(&self, nodes: &[Node], style: &Computed) -> Option<MixedText> {
        let owner = self.flow_of(nodes)?;
        let record = self.get(owner)?;
        let (prepared, source) = (record.prepared.as_ref()?, record.inline.as_ref()?);
        let text = self.normalized_inline(owner, style)?;
        text.items()
            .iter()
            .any(|item| !matches!(item.kind, ItemKind::Text { .. }))
            .then(|| MixedText {
                owner,
                source: source.clone(),
                text,
                inputs: prepared.dependencies.clone(),
            })
    }

    /// Stable T3 input: offsets address this UTF-8 buffer; BoxIds identify enclosing boxes.
    pub(crate) fn normalized_inline(&self, id: BoxId, style: &Computed) -> Option<InlineContent> {
        let record = self.get(id)?;
        let prepared = record.prepared.as_ref()?;
        if !(prepared.ready
            && prepared
                .inputs
                .matches(style, style.reset_text_box.ruby_suppress_breaks))
        {
            return None;
        }
        prepared.text.as_ref().or(record.inline.as_ref()).cloned()
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
        if !source.items().iter().all(|item| {
            matches!(
                item.kind,
                ItemKind::Text { .. } | ItemKind::Open | ItemKind::Close
            )
        }) {
            self.arena.prepared(id, None);
            return;
        }
        if record.prepared.as_ref().is_some_and(|p| {
            p.inputs
                .matches(style, style.reset_text_box.ruby_suppress_breaks)
                && mixed::matches(p.dependencies.as_deref().unwrap_or_default(), styles)
        }) {
            self.arena.prepared_ready(id, true);
            return;
        }
        let mut normalizer = Normalizer::default();
        let (mut text, mut items) = (String::new(), Vec::new());
        let mut dependencies = Vec::new();
        for item in source.items() {
            let Some(range) = item.kind.text_range() else {
                items.push(item.clone());
                continue;
            };
            let Some(input) = mixed::input(id, item.box_id, style, styles, &mut dependencies)
            else {
                // Keep the previous immutable owner while computed descendants
                // are pending, but never expose it as the current IFC input.
                self.arena.prepared_ready(id, false);
                return;
            };
            let run = normalizer.text(
                &source.text()[range],
                input,
                input.reset_text_box.ruby_suppress_breaks,
            );
            let start = inline::offset(text.len());
            text.push_str(&run);
            items.push(Item {
                box_id: item.box_id,
                kind: ItemKind::Text {
                    start,
                    end: inline::offset(text.len()),
                },
            });
        }
        let same = text == source.text() && items == source.items();
        let prepared = Prepared {
            ready: true,
            inputs: Inputs::of(style, style.reset_text_box.ruby_suppress_breaks),
            dependencies: (!dependencies.is_empty()).then(|| dependencies.into()),
            text: (!same).then(|| InlineContent::single(text, items)),
        };
        self.arena.prepared(id, Some(prepared));
    }
}

#[cfg(feature = "heap-report")]
impl kamin_heap::HeapSize for Prepared {
    fn heap_size(&self, ops: &mut kamin_heap::Ops) -> usize {
        // A shared list is counted by its first holder only.
        let dependencies = match self.dependencies.as_deref() {
            Some(shared) if ops.once(shared.as_ptr()) => shared,
            _ => &[],
        };
        self.inputs.flow_heap_size(ops)
            + self
                .text
                .as_ref()
                .map_or(0, |text| kamin_heap::HeapSize::heap_size(text, ops))
            + ops.buffer(dependencies.as_ptr(), dependencies.len())
            + dependencies
                .iter()
                .map(|(_, input)| input.flow_heap_size(ops))
                .sum::<usize>()
    }
}
