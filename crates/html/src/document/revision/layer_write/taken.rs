//! What the layer took of a restyle and the boxes it reaches along paths.

use super::super::patch::{Path, element_at};
use crate::dom::{Element, Node};

/// What the layer took of a restyle: the boxes the next frame records again
/// and the boxes off the screen whose records wait.
#[derive(Default)]
pub(in crate::document::revision) struct Taken {
    pub(in crate::document::revision) records: Vec<(Path, u64)>,
    pub(in crate::document::revision) deferred: Vec<(Path, u64)>,
}

/// The box numbered `id` at `path` in `nodes`; `None` when the path leads
/// elsewhere (the trees do not share their shape).
pub(in crate::document) fn at<'a>(
    nodes: &'a [Node],
    path: &[usize],
    id: u64,
) -> Option<&'a Element> {
    element_at(nodes, path).filter(|e| e.node_id == id)
}

/// The element holding the box at `path` in `nodes`.
pub(in crate::document) fn parent_at<'a>(nodes: &'a [Node], path: &[usize]) -> Option<&'a Element> {
    path.len()
        .checked_sub(1)
        .and_then(|n| element_at(nodes, &path[..n]))
}
