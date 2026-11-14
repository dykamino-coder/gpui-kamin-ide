//! Heap stacks of the paragraph piece collection, reused by the paragraphs of a document build.
//!
//! Every paragraph collects its pieces on a heap stack (`reblessive`), which grows by
//! power-of-two blocks and keeps its newest block until it is dropped. A new stack per
//! paragraph allocated and freed that block for every paragraph of the page (table-360:
//! 23.6 MB of blocks in one cold build), and a few bytes more in the collection's frames
//! doubled the block size. A stack given back after a paragraph serves the next one with
//! the block it already has, so the build allocates the deepest block once.
//!
//! The memory is layout state of the document (`LayoutState`), like `StyleConversions`, and
//! holds stacks only inside its builds ([`CollectStacks::build`]): outside builds a paragraph
//! takes a stack of its own, and the end of the outermost build frees the spares, so their
//! blocks never outlive the build.

use std::cell::Cell;

/// Spare stacks kept for nested paragraphs (an inline-block's paragraph inside a paragraph
/// takes a second stack while the first is in use).
const SPARES: usize = 4;

#[derive(Default)]
pub(crate) struct CollectStacks {
    /// In place, not a `Vec`: a build that keeps spares allocates nothing for the list.
    spare: [Cell<Option<reblessive::Stack>>; SPARES],
    /// Builds of the document in progress (a nested document builds inside its host's).
    builds: Cell<u8>,
}

/// A document build in progress: its paragraphs share spare stacks until it ends.
pub(crate) struct Build<'a>(&'a CollectStacks);

impl Drop for Build<'_> {
    fn drop(&mut self) {
        let left = self.0.builds.get().saturating_sub(1);
        self.0.builds.set(left);
        if left == 0 {
            for slot in &self.0.spare {
                drop(slot.take());
            }
        }
    }
}

impl CollectStacks {
    /// Start a build of the document; dropping the result ends it.
    pub(crate) fn build(&self) -> Build<'_> {
        self.builds.set(self.builds.get().saturating_add(1));
        Build(self)
    }

    /// Run `f` on a spare stack of the current build, or on a stack of its own outside builds.
    pub(crate) fn with_stack<R>(&self, f: impl FnOnce(&mut reblessive::Stack) -> R) -> R {
        let building = self.builds.get() > 0;
        let spare = building
            .then(|| self.spare.iter().rev().find_map(Cell::take))
            .flatten();
        let mut stack = spare.unwrap_or_default();
        let out = f(&mut stack);
        if building {
            let mut stack = Some(stack);
            for slot in &self.spare {
                let held = slot.take();
                if held.is_none() {
                    slot.set(stack.take());
                    break;
                }
                slot.set(held);
            }
        }
        out
    }

    #[cfg(test)]
    fn spares(&self) -> usize {
        self.spare
            .iter()
            .map(|slot| {
                let held = slot.take();
                let some = usize::from(held.is_some());
                slot.set(held);
                some
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_reuses_one_stack_and_frees_it_at_its_end() {
        let stacks = CollectStacks::default();
        assert_eq!(stacks.with_stack(|_| 1), 1);
        assert_eq!(stacks.spares(), 0, "outside builds a stack is not kept");
        {
            let _build = stacks.build();
            stacks.with_stack(|_| ());
            assert_eq!(stacks.spares(), 1);
            stacks.with_stack(|_| assert_eq!(stacks.spares(), 0, "the spare is in use"));
            assert_eq!(stacks.spares(), 1);
            {
                let _nested = stacks.build();
            }
            assert_eq!(
                stacks.spares(),
                1,
                "a nested build keeps the outer build's spares"
            );
        }
        assert_eq!(stacks.spares(), 0);
    }

    #[test]
    fn nested_paragraphs_take_their_own_stacks() {
        let stacks = CollectStacks::default();
        let _build = stacks.build();
        let depth = stacks.with_stack(|outer| {
            outer
                .enter(|_| async {
                    stacks.with_stack(|inner| inner.enter(|_| async { 2 }).finish())
                })
                .finish()
        });
        assert_eq!(depth, 2);
        assert_eq!(stacks.spares(), 2);
    }
}
