//! Контекст значения: документ, в арены которого разбор кладёт тела
//! `calc()`/`anchor()` и из которых их читает разрешение, и метрики корня
//! для `rem`/`rlh`.
//!
//! Его передают явно, как `ConstraintSpace` раскладки: разбор листа и каскад
//! берут его у документа (`SheetCx`, `TreeIndex`), сборка кадра и фазы GPUI —
//! у `LayoutCx`. Ручка `ArenaKey` читается только в документе, который её
//! выдал: чужая или устаревшая ручка — ошибка связи, а не чужое значение.

use super::DocumentValues;
use std::rc::Rc;

/// Документ значения и область записи его арен: разбор и каскад пишут в
/// область документа, сборка кадра — в область кадра.
#[derive(Clone, Copy)]
pub struct ValueCx<'a> {
    pub(crate) doc: &'a DocumentValues,
    pub(crate) frame: bool,
    pub(crate) viewport: (f32, f32),
    pub(crate) scheme: crate::style::values::value::Scheme,
}

impl DocumentValues {
    /// Контекст значений разбора и каскада документа.
    pub fn values(&self) -> ValueCx<'_> {
        ValueCx {
            doc: self,
            viewport: (0.0, 0.0),
            frame: false,
            scheme: crate::style::values::value::Scheme::new(self.media.get().dark),
        }
    }

    /// Контекст значений сборки кадра: записи арен живут до краски кадра.
    pub fn frame_values(&self) -> ValueCx<'_> {
        ValueCx {
            doc: self,
            viewport: (0.0, 0.0),
            frame: true,
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
    doc: Rc<DocumentValues>,
    frame: bool,
    viewport: (f32, f32),
    scheme: crate::style::values::value::Scheme,
}

impl ValueDoc {
    pub fn vcx(&self) -> ValueCx<'_> {
        ValueCx {
            doc: &self.doc,
            frame: self.frame,
            viewport: self.viewport,
            scheme: self.scheme,
        }
    }
}

impl<'a> ValueCx<'a> {
    pub(crate) fn with_viewport(mut self, viewport: (f32, f32)) -> Self {
        self.viewport = viewport;
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
    /// Удержать документ контекста. Нижние данные создаются только в `Rc`
    /// (`DocumentValues::new`), так что ручка есть, пока жив заимствованный
    /// документ; иначе (документ уже освобождается) — пустые данные без
    /// бэкенда шрифтов, и чтение его ручек даёт запасное значение.
    pub fn keep(self) -> ValueDoc {
        ValueDoc {
            doc: self
                .doc
                .this
                .upgrade()
                .unwrap_or_else(|| DocumentValues::detached(crate::style::css::Media::default())),
            frame: self.frame,
            viewport: self.viewport,
            scheme: self.scheme,
        }
    }
}
