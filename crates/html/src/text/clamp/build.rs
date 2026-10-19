//! Active clamp count root and paragraph numbers passed through layout builds.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct ClampBuild {
    pub(super) context: Option<(u64, bool)>,
    pub(super) seq: Rc<RefCell<HashMap<u64, u32>>>,
}

impl ClampBuild {
    /// A container starts its own numbering without changing the parent input.
    pub(crate) fn container(&self, key: u64) -> Self {
        self.seq.borrow_mut().insert(key, 0);
        Self {
            context: Some((key, false)),
            seq: self.seq.clone(),
        }
    }

    /// Lines in an independent BFC do not count against the ancestor's budget.
    pub(crate) fn bfc(&self) -> Self {
        Self {
            context: self.context.map(|(key, _)| (key, true)),
            seq: self.seq.clone(),
        }
    }

    /// Repeated measurement starts with the captured numbers, never outer ones.
    pub(super) fn snapshot(&self) -> Self {
        Self {
            context: self.context,
            seq: Rc::new(RefCell::new(self.seq.borrow().clone())),
        }
    }
}
