//! Прокрутка: владелец overflow, нативная цель и цепь sticky-проекций.

use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct ScrollContext {
    /// Scrollable overflow owner captured by probes and deferred text elements.
    pub(crate) frame: Option<crate::interactive::scroll_extent::Frame>,
    /// Native scroll handle offered to this CSS box by its scroll-area builder.
    pub(crate) target: Option<Rc<crate::interactive::scroll_target::Target>>,
    /// Chain of the sticky boxes and reversed scroll projections the box is
    /// built inside (`ShiftLink`): its scroll-extent probes subtract their
    /// paint-time shifts.
    pub(crate) sticky: Option<crate::interactive::scroll_extent::StickyShift>,
}
