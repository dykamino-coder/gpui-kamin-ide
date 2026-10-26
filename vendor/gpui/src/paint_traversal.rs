//! Общие callbacks обхода: порядок и семантика метаданных принадлежат хозяину.

use crate::{AnyElement, App, Style, Window};
use std::{any::Any, rc::Rc};

type ChildrenCallback = dyn Fn(&Style, &mut dyn Iterator<Item = &mut AnyElement>, &mut Window, &mut App);
type BoundaryCallback = dyn Fn(&mut Window) -> Box<dyn Any>;
type FinishRecording = dyn Fn(&mut dyn Any, &mut Window, &mut App);

/// Host-owned work that must finish inside a source command recording scope.
/// GPUI only pairs entry, completion and guard destruction; it owns no queue.
#[derive(Clone)]
pub struct PaintRecording {
    /// Open the host collector after entering the fresh recording scope.
    pub begin: Rc<BoundaryCallback>,
    /// Finish deferred host work before that scope and its resources close.
    pub finish: Rc<FinishRecording>,
}

/// KaminIDE patch: visits children exactly at the normal child-paint location inside a box.
/// The callback must paint each child at most once and must not repeat prepaint.
#[derive(Clone)]
pub struct ChildPainter(pub Rc<ChildrenCallback>);

/// Optional host traversal, installed with `Window::with_render_context`.
/// The boundary guard restores the host scope even if an element unwinds.
pub struct PaintTraversal {
    /// Enter an opaque element; GPUI never interprets the returned guard.
    pub enter_boundary: Rc<BoundaryCallback>,
    /// Default child visitor for boxes without a local override.
    pub children: ChildPainter,
    /// Optional lifecycle for source recording; playback never invokes it.
    pub recording: Option<PaintRecording>,
}
