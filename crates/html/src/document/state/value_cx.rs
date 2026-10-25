//! Контекст значения: документ, в арены которого разбор кладёт тела
//! `calc()`/`anchor()` и из которых их читает разрешение, и метрики корня
//! для `rem`/`rlh`.
//!
//! Его передают явно, как `ConstraintSpace` раскладки: разбор листа и каскад
//! берут его у документа (`SheetCx`, `TreeIndex`), сборка кадра и фазы GPUI —
//! у `LayoutCx`. Ручка `ArenaKey` читается только в документе, который её
//! выдал: чужая или устаревшая ручка — ошибка связи, а не чужое значение.

use super::DocState;
use std::{cell::RefCell, rc::Rc};

/// Документ значения и область записи его арен: разбор и каскад пишут в
/// область документа, сборка кадра — в область кадра.
#[derive(Clone, Copy)]
pub struct ValueCx<'a> {
    pub(crate) doc: &'a DocState,
    pub(crate) frame: bool,
    pub(crate) lengths: Option<&'a Rc<RefCell<crate::layout::fragment::line_shape::LengthFacts>>>,
    pub(crate) viewport: (f32, f32),
    pub(crate) lines: Option<&'a Rc<crate::layout::fragment::LineCx>>,
    pub(crate) paint: Option<&'a Rc<crate::layout::cx::deferred::Deferred>>,
    pub(crate) collapse: crate::layout::block::margins::CollapseInputs,
    pub(crate) scheme: crate::style::values::value::Scheme,
}

impl DocState {
    /// Контекст значений разбора и каскада документа.
    pub fn values(&self) -> ValueCx<'_> {
        ValueCx {
            doc: self,
            viewport: (0.0, 0.0),
            lines: None,
            paint: None,
            frame: false,
            lengths: None,
            collapse: Default::default(),
            scheme: crate::style::values::value::Scheme::new(self.media.get().dark),
        }
    }

    /// Контекст значений сборки кадра: записи арен живут до краски кадра.
    pub fn frame_values(&self) -> ValueCx<'_> {
        ValueCx {
            doc: self,
            viewport: (0.0, 0.0),
            lines: None,
            paint: None,
            frame: true,
            lengths: None,
            collapse: Default::default(),
            scheme: crate::style::values::value::Scheme::new(self.media.get().dark),
        }
    }
}

impl std::fmt::Debug for ValueCx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValueCx")
            .field("frame", &self.frame)
            .finish()
    }
}

/// Владеющий контекст значения — для того, что живёт дольше вызова: замыкания
/// и обёртки фаз GPUI (подготовка, краска) держат документ сами.
#[derive(Clone)]
pub struct ValueDoc {
    doc: Rc<DocState>,
    frame: bool,
    lengths: Option<Rc<RefCell<crate::layout::fragment::line_shape::LengthFacts>>>,
    viewport: (f32, f32),
    lines: Option<Rc<crate::layout::fragment::LineCx>>,
    paint: Option<Rc<crate::layout::cx::deferred::Deferred>>,
    collapse: crate::layout::block::margins::CollapseInputs,
    scheme: crate::style::values::value::Scheme,
}

impl ValueDoc {
    pub fn vcx(&self) -> ValueCx<'_> {
        ValueCx {
            doc: &self.doc,
            lines: self.lines.as_ref(),
            paint: self.paint.as_ref(),
            frame: self.frame,
            lengths: self.lengths.as_ref(),
            viewport: self.viewport,
            collapse: self.collapse,
            scheme: self.scheme,
        }
    }
}

impl<'a> ValueCx<'a> {
    pub(crate) fn with_lengths(
        mut self,
        facts: &'a Rc<RefCell<crate::layout::fragment::line_shape::LengthFacts>>,
    ) -> Self {
        self.lengths = Some(facts);
        self
    }
    pub(crate) fn with_viewport(mut self, viewport: (f32, f32)) -> Self {
        self.viewport = viewport;
        self
    }

    pub(crate) fn with_paint(
        mut self,
        paint: Option<&'a Rc<crate::layout::cx::deferred::Deferred>>,
    ) -> Self {
        self.paint = paint;
        self
    }

    pub(crate) fn with_lines(
        mut self,
        lines: Option<&'a Rc<crate::layout::fragment::LineCx>>,
    ) -> Self {
        self.lines = lines;
        self
    }

    pub(crate) fn with_collapse(
        mut self,
        inputs: crate::layout::block::margins::CollapseInputs,
    ) -> Self {
        self.collapse = inputs;
        self
    }

    /// The same context storing into the document arenas: values that outlive
    /// the frame (materialized computed styles) must not hold frame records.
    pub(crate) fn persistent(mut self) -> Self {
        self.frame = false;
        self
    }

    pub(crate) fn with_scheme(mut self, scheme: crate::style::values::value::Scheme) -> Self {
        self.scheme = scheme;
        self
    }
    /// Удержать документ контекста. Документ всегда живёт в `Rc`
    /// (`DocState::new`), так что ручка есть, пока жив заимствованный
    /// документ; иначе — пустой документ, и чтение его ручек даёт запасное
    /// значение.
    pub fn keep(self) -> ValueDoc {
        ValueDoc {
            doc: self
                .doc
                .this
                .upgrade()
                .unwrap_or_else(|| DocState::new(crate::style::css::Media::default())),
            frame: self.frame,
            lengths: self.lengths.cloned(),
            viewport: self.viewport,
            collapse: self.collapse,
            scheme: self.scheme,
            lines: self.lines.cloned(),
            paint: self.paint.cloned(),
        }
    }
}
