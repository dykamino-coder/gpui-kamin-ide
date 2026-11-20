//! Why the frame record was built instead of replayed: one counter per reason,
//! so a page that builds every scroll step names the check that refused it.

use gpui::ScrollRefusal;

/// The check that refused a replay of the frame record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::document::retained) enum FrameMiss {
    /// Document resources changed since the record (images, fonts).
    Resources,
    /// No record was published: none yet, or its element state was dropped.
    Unpublished,
    /// The record was painted under a transform, opacity, mask or group.
    NotRoot,
    /// Its commands cannot be copied (an isolated scene, a layer's group).
    Commands,
    /// Its paint scheduled frame callbacks (animations, deferred notifications).
    Scheduled,
    /// Document values were not ready for the frame (`frame_ready`).
    NotReady,
    /// A deferred draw painted a composition group after the record.
    Unreplayable,
    /// The layout read a scroll offset that changed while recording.
    ScrollRead,
    /// The frame key changed (document generation, options, viewport).
    Key,
    /// The element state holds another record than the published one.
    Stale,
    /// The window, frame serial, root environment or window key changed.
    Environment,
    /// Laid out against a scroll offset that moved (`layout_scroll_replayable`).
    LayoutScroll,
    /// The rendered frame's ranges cannot be replayed (`retained_replayable_with_groups`).
    Paint,
    /// Boxes queued for repaint could not be recorded again.
    Boxes,
    /// The scrolled replay was refused (`gpui::Window::scroll_replay_plan`).
    Scroll(ScrollRefusal),
}

const SCROLL: [ScrollRefusal; ScrollRefusal::ALL.len()] = ScrollRefusal::ALL;

/// Reasons before the scrolled replay refusals, in counter order.
const PLAIN: [(FrameMiss, &str); 14] = [
    (FrameMiss::Resources, "resources"),
    (FrameMiss::Unpublished, "unpublished"),
    (FrameMiss::NotRoot, "not_root"),
    (FrameMiss::Commands, "commands"),
    (FrameMiss::Scheduled, "scheduled"),
    (FrameMiss::NotReady, "not_ready"),
    (FrameMiss::Unreplayable, "unreplayable"),
    (FrameMiss::ScrollRead, "scroll_read"),
    (FrameMiss::Key, "key"),
    (FrameMiss::Stale, "stale"),
    (FrameMiss::Environment, "environment"),
    (FrameMiss::LayoutScroll, "layout_scroll"),
    (FrameMiss::Paint, "paint"),
    (FrameMiss::Boxes, "boxes"),
];

/// Number of counters: the plain reasons, then one per scrolled refusal.
pub(in crate::document::retained) const MISSES: usize = PLAIN.len() + SCROLL.len();

impl FrameMiss {
    fn index(self) -> usize {
        match self {
            FrameMiss::Scroll(refusal) => {
                PLAIN.len() + SCROLL.iter().position(|r| *r == refusal).unwrap_or(0)
            }
            plain => PLAIN.iter().position(|(m, _)| *m == plain).unwrap_or(0),
        }
    }

    /// The counter name of reason `index`.
    pub(in crate::document::retained) fn name(index: usize) -> String {
        match PLAIN.get(index) {
            Some((_, name)) => (*name).into(),
            None => format!("scroll_{:?}", SCROLL[index - PLAIN.len()]).to_lowercase(),
        }
    }

    /// Count one refusal in `counters`.
    pub(in crate::document::retained) fn count(self, counters: &std::cell::Cell<[u64; MISSES]>) {
        let mut all = counters.get();
        all[self.index()] += 1;
        counters.set(all);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_its_own_counter_and_name() {
        let counters = std::cell::Cell::new([0; MISSES]);
        let reasons = PLAIN
            .iter()
            .map(|(m, _)| *m)
            .chain(SCROLL.iter().map(|r| FrameMiss::Scroll(*r)));
        for reason in reasons {
            reason.count(&counters);
        }
        assert_eq!(counters.get(), [1; MISSES]);
        let names: std::collections::HashSet<_> = (0..MISSES).map(FrameMiss::name).collect();
        assert_eq!(names.len(), MISSES);
        assert_eq!(FrameMiss::name(PLAIN.len() + 4), "scroll_interest");
    }
}
