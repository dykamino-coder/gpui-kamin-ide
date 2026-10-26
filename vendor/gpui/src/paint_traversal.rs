//! Общие callbacks обхода: порядок и семантика метаданных принадлежат хозяину.

use crate::{AnyElement, App, Style, Window};
use std::{any::Any, rc::Rc};

type ChildrenCallback = dyn Fn(&Style, &mut dyn Iterator<Item = &mut AnyElement>, &mut Window, &mut App);
type BoundaryCallback = dyn Fn(&mut Window) -> Box<dyn Any>;

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
}
