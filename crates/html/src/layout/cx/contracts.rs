//! Контракт сборки: функции оркестратора и границы вложенных документов.

#[derive(Clone, Default)]
pub(crate) struct BuildContext {
    /// Child-build requests of the render orchestrator (`ChildBuild`).
    pub(crate) build: crate::layout::build_contracts::ChildBuild,
    /// How many documents (iframes, SVG document images) the box is nested
    /// in: a guard against cyclic embedding.
    pub(crate) frame_depth: u32,
    /// Built inside a `render` call: an inner `render` must not reset the
    /// step-8 paint collector of the outer one.
    pub(crate) in_render: bool,
    /// Корневой список, неизменность которого гарантирует Document.
    pub(crate) document_root: Option<(usize, usize)>,
    /// Document::frame_opts уже начал кадр измерений текста этого документа.
    pub(crate) paragraph_frame_started: bool,
    /// Семантические коробки на пути сборки; не число служебных обёрток GPUI.
    pub(crate) reflow_depth: usize,
    /// Факты неизменяемых поддеревьев живут только в пределах одной сборки.
    pub(crate) subtrees: std::rc::Rc<std::cell::RefCell<crate::layout::cx::subtree::SubtreeFacts>>,
    pub(crate) lengths:
        std::rc::Rc<std::cell::RefCell<crate::layout::fragment::line_shape::LengthFacts>>,
}
