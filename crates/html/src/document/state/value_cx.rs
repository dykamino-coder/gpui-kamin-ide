//! Контекст значения: документ, в арены которого разбор кладёт тела
//! `calc()`/`anchor()` и из которых их читает разрешение, и метрики корня
//! для `rem`/`rlh`.
//!
//! Его передают явно, как `ConstraintSpace` раскладки: разбор листа и каскад
//! берут его у документа (`SheetCx`, `TreeIndex`), сборка кадра и фазы GPUI —
//! у `LayoutCx`. Ручка `ArenaKey` читается только в документе, который её
//! выдал: чужая или устаревшая ручка — ошибка связи, а не чужое значение.

use super::DocState;
use std::rc::Rc;

/// Документ значения и область записи его арен: разбор и каскад пишут в
/// область документа, сборка кадра — в область кадра.
#[derive(Clone, Copy)]
pub struct ValueCx<'a> {
    pub(crate) doc: &'a DocState,
    pub(crate) frame: bool,
}

impl DocState {
    /// Контекст значений разбора и каскада документа.
    pub fn values(&self) -> ValueCx<'_> {
        ValueCx {
            doc: self,
            frame: false,
        }
    }

    /// Контекст значений сборки кадра: записи арен живут до краски кадра.
    pub fn frame_values(&self) -> ValueCx<'_> {
        ValueCx {
            doc: self,
            frame: true,
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
}

impl ValueDoc {
    pub fn vcx(&self) -> ValueCx<'_> {
        ValueCx {
            doc: &self.doc,
            frame: self.frame,
        }
    }
}

impl ValueCx<'_> {
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
        }
    }
}
