//! Heap stacks of the piece collection, reused by the paragraphs of a document build.
//!
//! Every paragraph collects its pieces on a heap stack (`reblessive`), which grows by
//! power-of-two blocks and keeps its newest block until it is dropped. A new stack per
//! paragraph allocated and freed that block for every paragraph of the page (table-360:
//! 23.6 MB of blocks in one cold build), and a few bytes more in the collection's frames
//! doubled the block size. A stack given back after a paragraph serves the next one with
//! the block it already has, so the build allocates the deepest block once.
//!
//! The spare stacks live only while a document is being built ([`build`]); outside builds a
//! paragraph takes a stack of its own as before, and the end of the outermost build frees the
//! spares, so their blocks never outlive the build.

use std::cell::{Cell, RefCell};

/// Spare stacks kept for nested paragraphs (an inline-block's paragraph inside a paragraph
/// takes a second stack while the first is in use).
const SPARES: usize = 4;

thread_local! {
    /// In place, not a `Vec`: a build that keeps spares allocates nothing for the list.
    static SPARE: RefCell<[Option<reblessive::Stack>; SPARES]> =
        const { RefCell::new([const { None }; SPARES]) };
    static BUILDS: Cell<u32> = const { Cell::new(0) };
}

/// A document build in progress: its paragraphs share spare stacks until it ends.
pub(crate) struct Build(());

impl Drop for Build {
    fn drop(&mut self) {
        let builds = BUILDS.with(|b| {
            let left = b.get().saturating_sub(1);
            b.set(left);
            left
        });
        if builds == 0 {
            let spare =
                SPARE.with(|s| std::mem::replace(&mut *s.borrow_mut(), [const { None }; SPARES]));
            drop(spare);
        }
    }
}

/// Start a document build; dropping the result ends it.
pub(crate) fn build() -> Build {
    BUILDS.with(|b| b.set(b.get().saturating_add(1)));
    Build(())
}

/// Run `f` on a spare stack of the current build, or on a stack of its own outside builds.
pub(super) fn with_stack<R>(f: impl FnOnce(&mut reblessive::Stack) -> R) -> R {
    let building = BUILDS.with(Cell::get) > 0;
    let spare = building
        .then(|| SPARE.with(|s| s.borrow_mut().iter_mut().rev().find_map(Option::take)))
        .flatten();
    let mut stack = spare.unwrap_or_default();
    let out = f(&mut stack);
    if building {
        SPARE.with(|s| {
            if let Some(slot) = s.borrow_mut().iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(stack);
            }
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spares() -> usize {
        SPARE.with(|s| s.borrow().iter().flatten().count())
    }

    #[test]
    fn a_build_reuses_one_stack_and_frees_it_at_its_end() {
        assert_eq!(with_stack(|_| 1), 1);
        assert_eq!(spares(), 0, "outside builds a stack is not kept");
        {
            let _build = build();
            with_stack(|_| ());
            assert_eq!(spares(), 1);
            with_stack(|_| assert_eq!(spares(), 0, "the spare is in use"));
            assert_eq!(spares(), 1);
            {
                let _nested = build();
            }
            assert_eq!(spares(), 1, "a nested build keeps the outer build's spares");
        }
        assert_eq!(spares(), 0);
    }

    #[test]
    fn nested_paragraphs_take_their_own_stacks() {
        let _build = build();
        let depth = with_stack(|outer| {
            outer
                .enter(|_| async { with_stack(|inner| inner.enter(|_| async { 2 }).finish()) })
                .finish()
        });
        assert_eq!(depth, 2);
        assert_eq!(spares(), 2);
    }
}
