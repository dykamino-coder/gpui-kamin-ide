//! Frame counters belong to a retained tree, which a restyle can replace.

use super::Slot;
use crate::Document;
use std::rc::{Rc, Weak};

/// A counter checkpoint without retaining the tree or its prepared elements.
/// Comparing checkpoints of different trees counts the new tree from zero.
pub struct FrameCounters {
    slot: Weak<Slot>,
    counts: (u64, u64, u64),
}

impl FrameCounters {
    /// Builds, replays and builds inside draw since `before`. `None` means
    /// a counter of the same tree regressed; a new tree starts at zero.
    pub fn since(&self, before: &Self) -> Option<(u64, u64, u64)> {
        let before = if Weak::ptr_eq(&self.slot, &before.slot) {
            before.counts
        } else {
            (0, 0, 0)
        };
        Some((
            self.counts.0.checked_sub(before.0)?,
            self.counts.1.checked_sub(before.1)?,
            self.counts.2.checked_sub(before.2)?,
        ))
    }
}

impl Document {
    /// Checkpoint the retained frame counters, including their tree identity.
    /// A viewport media query may replace that tree without a host edit.
    pub fn frame_counters(&self) -> FrameCounters {
        FrameCounters {
            slot: Rc::downgrade(&self.retained),
            counts: (
                self.retained.builds.get(),
                self.retained.replays.get(),
                self.retained.inside_draw.get(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn a_media_restyle_counts_the_new_frame_from_zero(cx: &mut gpui::TestAppContext) {
        use super::super::tests::{frame, open};
        let (view, cx) = open(
            cx,
            "<style>main{container-type:inline-size}\
            @media(width > 400px){main{color:red}}\
            @container(width > 200px){p{color:blue}}</style><main><p>text</p></main>",
        );
        frame(cx);
        let before = cx.update(|_, app| view.read(app).doc.frame_counters());
        cx.update(|_, app| view.update(app, |view, _| view.viewport = (401., 300.)));
        frame(cx);
        let after = cx.update(|_, app| view.read(app).doc.frame_counters());
        assert!(
            !Weak::ptr_eq(&before.slot, &after.slot),
            "media restyle replaces the tree"
        );
        let (built, replayed, _) = after.since(&before).unwrap();
        assert!(built > 0);
        assert_eq!(replayed, 0);
    }

    fn snapshot(slot: &Rc<Slot>, counts: (u64, u64, u64)) -> FrameCounters {
        FrameCounters {
            slot: Rc::downgrade(slot),
            counts,
        }
    }

    #[test]
    fn equal_counts_of_different_trees_are_not_zero_work() {
        let old = Rc::new(Slot::default());
        let next = Rc::new(Slot::default());
        let before = snapshot(&old, (2, 0, 1));
        let after = snapshot(&next, (2, 0, 1));
        assert_eq!(after.since(&before), Some((2, 0, 1)));
        drop(old);
        assert_eq!(after.since(&before), Some((2, 0, 1)));
    }

    #[test]
    fn same_tree_counts_only_new_work_and_rejects_regression() {
        let slot = Rc::new(Slot::default());
        let before = snapshot(&slot, (3, 2, 1));
        assert_eq!(snapshot(&slot, (5, 3, 2)).since(&before), Some((2, 1, 1)));
        assert_eq!(snapshot(&slot, (2, 2, 1)).since(&before), None);
    }
}
