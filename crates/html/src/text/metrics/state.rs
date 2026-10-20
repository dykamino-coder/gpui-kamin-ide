//! Font metrics and measurement callbacks belonging to one document.

use super::{Probe, VProbe};
use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub(crate) struct Metrics {
    pub(super) family: RefCell<String>,
    pub(super) probe: RefCell<Option<Probe>>,
    pub(super) vprobe: RefCell<Option<VProbe>>,
    pub(super) cache: RefCell<HashMap<String, (f32, f32, f32, f32)>>,
    pub(super) vcache: RefCell<HashMap<String, (f32, f32, f32)>>,
    pub(super) parts: RefCell<HashMap<u32, (f32, f32, f32)>>,
}
