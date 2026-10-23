//! Ruby: измерения базы и общие слои аннотаций текущего строчного контекста.

use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct RubyContext {
    /// Base-width sink of the ruby column being built: the first paragraph
    /// built under it takes it and reports its max-content width (ruby
    /// overhang, css-ruby-1 §4.4).
    pub(crate) base: Option<crate::text::paragraph::RubyBaseOffer>,
    /// Annotation layers of the ruby base being built: nested ruby
    /// annotations stack on them.
    pub(crate) layers: Option<Rc<crate::text::ruby::layers::Layers>>,
    /// Collection of the annotation stacks of the inline atom being built
    /// (the line grows by them); `None` inside — collection closed.
    pub(crate) extents:
        Option<std::rc::Rc<std::cell::RefCell<Option<crate::text::paragraph::RubyExtents>>>>,
}
