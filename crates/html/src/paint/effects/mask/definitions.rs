//! Definitions and retained snapshots resolved only within their owning document.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct MaskDefinitions {
    pub(super) defs: RefCell<HashMap<String, String>>,
    pub(super) alpha: RefCell<HashSet<String>>,
    pub(super) nodes: Cell<usize>,
    pub(super) snaps: RefCell<HashMap<String, String>>,
    pub(super) next: Cell<u64>,
}
