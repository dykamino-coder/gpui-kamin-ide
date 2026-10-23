//! Отложенная отрисовка: общие владельцы PaintCx и paint-order текущего дерева.

use std::rc::Rc;

#[derive(Clone, Default)]
pub struct DeferredPaintContext {
    /// Prepaint frame shared only by elements of this render tree.
    /// Traversal state captured by this render tree's deferred painters.
    pub(crate) paint: Rc<crate::paint::context::PaintCx>,
    pub(crate) vertical_paint: Rc<crate::text::vertical::VerticalPaint>,
    /// Paint-order and deferred-geometry boundaries of the box being built.
    pub(crate) depth: crate::paint::effects::paint_scope::Depth,
    /// Paint keys of CSS 2.1 Appendix E step 8 (`gpui::PaintLast`), in build
    /// order: an ancestor gets a smaller key than its descendants. Shared by
    /// everything that paints into one collector, nested documents included.
    pub(crate) keys: Rc<std::cell::Cell<u64>>,
    /// Positions of the document's nodes in a forward walk and of the LAST
    /// positioned node that keeps no paint key (`unkeyed_positions`).
    pub(crate) unkeyed: Rc<crate::layout::order::Unkeyed>,
    /// Nearest gap-decorated container collecting child bounds.
    pub(crate) gap_items: Option<crate::paint::gap_rules::GapItems>,
    /// Keep scrollbar hover and drag at rest for a static host capture.
    pub freeze_scrollbars: bool,
}
