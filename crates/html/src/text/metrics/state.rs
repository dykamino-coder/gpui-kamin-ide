//! Метрики шрифтов и колбэки измерения, принадлежащие одному документу.

use super::{FontContext, Probe, VProbe, WrapProbe};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

#[derive(Default)]
pub(crate) struct Metrics {
    pub(super) platform: RefCell<FontContext>,
    pub(super) wrap: RefCell<Option<WrapProbe>>,
    pub(super) space: RefCell<Option<super::space::SpaceProbe>>,
    pub(super) family: RefCell<String>,
    pub(super) probe: RefCell<Option<Probe>>,
    pub(super) vprobe: RefCell<Option<VProbe>>,
    pub(super) cache: RefCell<HashMap<String, (f32, f32, f32, f32)>>,
    pub(super) vcache: RefCell<HashMap<String, (f32, f32, f32)>>,
    pub(super) parts: RefCell<HashMap<u32, (f32, f32, f32)>>,
    /// Растёт при каждой смене того, от чего зависят шрифтовые единицы
    /// вычисленного стиля (`ch`/`ex`/`lh`, `line-height: normal`): семейства
    /// документа, щупов и платформенного контекста шрифтов.
    pub(super) generation: Cell<u64>,
}

impl Metrics {
    pub(crate) fn default_family(&self) -> String {
        self.family.borrow().clone()
    }

    pub(crate) fn font_context(&self) -> FontContext {
        self.platform.borrow().clone()
    }

    /// Поколение шрифтовых входов вычисленного стиля
    /// (`style::cascade::materialize`).
    pub(crate) fn generation(&self) -> u64 {
        self.generation.get()
    }

    pub(crate) fn invalidate(&self) {
        self.generation.set(self.generation.get() + 1);
    }
}
