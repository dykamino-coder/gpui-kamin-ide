//! Build scope retained by deferred layout children of a clamp container.
//!
//! Paragraph numbers are assigned in box-tree order (CSS Overflow 4 section
//! 5.3). Rebuilding a measured child must reuse those numbers without consuming
//! the surrounding container's sequence or losing its BFC counting boundary.

use super::line_state::{CLAMP_SEQ, CLAMP_STACK, clamp_context};
use std::collections::HashMap;

#[derive(Clone)]
pub(crate) struct DeferredClamp {
    stack: Vec<(u64, bool)>,
    seq: HashMap<u64, u32>,
    // Atomic depth, paragraph tag and budget travel in the builder's LayoutCx.
}

impl DeferredClamp {
    pub(crate) fn capture() -> Option<Self> {
        clamp_context()?;
        Some(Self::snapshot())
    }

    fn snapshot() -> Self {
        Self {
            stack: CLAMP_STACK.with_borrow(Clone::clone),
            seq: CLAMP_SEQ.with_borrow(Clone::clone),
        }
    }

    fn install(&self) {
        CLAMP_STACK.with_borrow_mut(|v| v.clone_from(&self.stack));
        CLAMP_SEQ.with_borrow_mut(|v| v.clone_from(&self.seq));
    }

    pub(crate) fn enter(&self) -> DeferredClampGuard {
        let previous = Self::snapshot();
        self.install();
        DeferredClampGuard(previous)
    }
}

pub(crate) struct DeferredClampGuard(DeferredClamp);

impl Drop for DeferredClampGuard {
    fn drop(&mut self) {
        self.0.install();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::clamp::{ClampGuard, clamp_next_seq};

    #[test]
    fn repeated_builds_reuse_paragraph_numbers_and_restore_outer_scope() {
        let scope = ClampGuard::enter(900);
        assert_eq!(clamp_next_seq(900), 0);
        let deferred = DeferredClamp::capture().expect("active container");
        // Reserve the deferred child's paragraphs during the tree build.
        assert_eq!(clamp_next_seq(900), 1);
        assert_eq!(clamp_next_seq(900), 2);
        drop(scope);
        let _outer = ClampGuard::enter(901);
        assert_eq!(clamp_next_seq(901), 0);
        for _ in 0..2 {
            let _build = deferred.enter();
            assert_eq!(clamp_context(), Some((900, false)));
            assert_eq!(clamp_next_seq(900), 1);
            assert_eq!(clamp_next_seq(900), 2);
        }
        assert_eq!(clamp_context(), Some((901, false)));
        assert_eq!(clamp_next_seq(901), 1);
        assert_eq!(super::super::line_state::clamp_peek_seq(900), 3);
    }

    #[test]
    fn deferred_build_retains_bfc_boundary() {
        let scope = ClampGuard::enter(902);
        let bfc = ClampGuard::enter_bfc();
        let deferred = DeferredClamp::capture().expect("active container");
        drop(bfc);
        drop(scope);
        {
            let _build = deferred.enter();
            assert_eq!(clamp_context(), Some((902, true)));
        }
        assert_eq!(clamp_context(), None);
    }
}
