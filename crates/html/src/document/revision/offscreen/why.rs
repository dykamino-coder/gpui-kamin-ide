//! What DevTools `why` says of an edit off the screen whose area is unknown:
//! which fact the layer's frame left out.

use super::{Wait, container};
use crate::document::boxes::BoxTree;
use crate::document::revision::patch::{Path, chain};
use crate::dom::{Element, Node};
use crate::layout::reuse::Store;
use gpui::LayerHandle;

/// Why no `kind` area of box `a` (at `path` in `old`) is known.
pub(super) fn missing(
    kind: Wait,
    (store, boxes, layer): (&Store, &BoxTree, &LayerHandle),
    (old, path): (&[Node], &Path),
    a: &Element,
) -> String {
    match kind {
        Wait::Inline => unknown_fragments(store, boxes, a, || {
            // The nearest element on the path a group or a run of its own holds.
            let parents = &path[..path.len().saturating_sub(1)];
            let up = chain(old, parents);
            let held = up.into_iter().rev().find_map(|e| {
                let owner = u32::try_from(e.node_id).ok()?;
                (!layer.run_painters(owner).is_empty()).then_some((e, owner))
            });
            match held {
                Some((e, owner)) => format!(
                    "<{}> {owner}: runs {:?}, group root {}, area {:?}, no run {:?}",
                    e.tag,
                    layer.run_painters(owner),
                    layer.holds_group_root(owner),
                    layer.box_area(owner),
                    layer.no_run(owner)
                ),
                None => "no run on the path".into(),
            }
        }),
        Wait::Decoration => "no decorated run in the layer".into(),
    }
}

/// Why `fragments_of` knows no fragments of inline box `a`, and what holds
/// the nearest ancestor with a run in the layer (`held`).
fn unknown_fragments(
    store: &Store,
    boxes: &BoxTree,
    a: &Element,
    held: impl FnOnce() -> String,
) -> String {
    let Some(id) = boxes.box_of(a.node_id) else {
        return "no box".into();
    };
    let Some(container) = container(boxes, id) else {
        return "no inline container".into();
    };
    match boxes.slot(container) {
        None => format!(
            "the container has no layout slot (its paragraphs wrote no fragments; {}): {}",
            held(),
            boxes.describe_chain(id, 6)
        ),
        Some(s) => match s.inline.as_deref() {
            // A paragraph painted in a group or under a transform measures
            // no ink in window space (`layout::retained::measured`).
            None => format!(
                "the container's paragraphs wrote no fragments, ink {:?}; {}",
                s.ink,
                held()
            ),
            Some(f) if !store.fragments_hold(f.stamp()) => {
                format!("fragments of frame {} do not describe the layer", f.stamp())
            }
            Some(_) => "no band owner and no lines hold the box".into(),
        },
    }
}
