//! Метрики шрифтов и колбэки измерения, принадлежащие одному документу.

use super::{FontContext, Probe, VProbe, WrapProbe};
use std::{cell::RefCell, collections::HashMap};

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
}

impl Metrics {
    pub(crate) fn font_context(&self) -> FontContext {
        self.platform.borrow().clone()
    }
}
