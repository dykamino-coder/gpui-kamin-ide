//! Boxes whose paint the document skips: an ancestor skips its contents
//! (`content-visibility: hidden`, the slot of a closed `details`), so no
//! paint of theirs reaches any frame (css-contain-2 §4: the contents are not
//! painted, as if `visibility: hidden` and more). A paint-only edit there
//! changes no pixel and needs neither a write nor a frame; the frame that
//! shows the contents again lays them out and paints them anew.

use crate::dom::Node;

/// Whether an ancestor of the box at `path` in `nodes` skips its contents.
pub(super) fn skipped(nodes: &[Node], path: &[usize]) -> bool {
    (1..path.len()).any(|k| {
        super::super::patch::element_at(nodes, &path[..k])
            .is_some_and(|e| e.style.used().reset_context.skip_content == Some(true))
    })
}
