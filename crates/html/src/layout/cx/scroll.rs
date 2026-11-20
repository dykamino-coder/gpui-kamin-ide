//! Прокрутка: владелец overflow, нативная цель и цепь sticky-проекций.

use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct ScrollContext {
    /// Scrollable overflow owner captured by probes and deferred text elements.
    pub(crate) frame: Option<crate::interactive::scroll_extent::Frame>,
    /// Native scroll handle offered to this CSS box by its scroll-area builder.
    pub(crate) target: Option<Rc<crate::interactive::scroll_target::Target>>,
    /// Native scroll node whose content the box is built in: the innermost
    /// scroll container around it, `None` outside any (or under a box hung
    /// on the viewport). A containing-block layer opened here records it, so
    /// a box laid out in the layer takes the layer's coordinates
    /// (`Positioning::layer_node`).
    pub(crate) node: Option<gpui::ScrollHandle>,
    /// Chain of the sticky boxes and reversed scroll projections the box is
    /// built inside (`ShiftLink`): its scroll-extent probes subtract their
    /// paint-time shifts.
    pub(crate) sticky: Option<crate::interactive::scroll_extent::StickyShift>,
}
