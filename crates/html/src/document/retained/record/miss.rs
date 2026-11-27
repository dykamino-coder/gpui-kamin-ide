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
const PLAIN: [(FrameMiss, &str); 13] = [
    (FrameMiss::Resources, "resources"),
    (FrameMiss::Unpublished, "unpublished"),
    (FrameMiss::NotRoot, "not_root"),
    (FrameMiss::Commands, "commands"),
    (FrameMiss::Scheduled, "scheduled"),
    (FrameMiss::NotReady, "not_ready"),
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

/// Whether `code` is the `why` code of a frame miss (`frame_miss.<counter name>`).
pub(crate) fn is_code(code: &str) -> bool {
    code.strip_prefix("frame_miss.")
        .is_some_and(|name| (0..MISSES).any(|at| FrameMiss::name(at) == name))
}

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

    /// The stable DevTools code of the reason (`why` journal): `frame_miss.` and its counter name.
    pub(in crate::document::retained) fn code(self) -> &'static str {
        match self {
            FrameMiss::Resources => "frame_miss.resources",
            FrameMiss::Unpublished => "frame_miss.unpublished",
            FrameMiss::NotRoot => "frame_miss.not_root",
            FrameMiss::Commands => "frame_miss.commands",
            FrameMiss::Scheduled => "frame_miss.scheduled",
            FrameMiss::NotReady => "frame_miss.not_ready",
            FrameMiss::ScrollRead => "frame_miss.scroll_read",
            FrameMiss::Key => "frame_miss.key",
            FrameMiss::Stale => "frame_miss.stale",
            FrameMiss::Environment => "frame_miss.environment",
            FrameMiss::LayoutScroll => "frame_miss.layout_scroll",
            FrameMiss::Paint => "frame_miss.paint",
            FrameMiss::Boxes => "frame_miss.boxes",
            FrameMiss::Scroll(refusal) => match refusal {
                ScrollRefusal::Environment => "frame_miss.scroll_environment",
                ScrollRefusal::DeferredDraws => "frame_miss.scroll_deferreddraws",
                ScrollRefusal::Node => "frame_miss.scroll_node",
                ScrollRefusal::NodeRange => "frame_miss.scroll_noderange",
                ScrollRefusal::Interest => "frame_miss.scroll_interest",
                ScrollRefusal::Dependent => "frame_miss.scroll_dependent",
                ScrollRefusal::Groups => "frame_miss.scroll_groups",
                ScrollRefusal::BottomLayer => "frame_miss.scroll_bottomlayer",
                ScrollRefusal::DepthSort => "frame_miss.scroll_depthsort",
                ScrollRefusal::UnbalancedLayer => "frame_miss.scroll_unbalancedlayer",
                ScrollRefusal::ForeignMark => "frame_miss.scroll_foreignmark",
                ScrollRefusal::Unmarked => "frame_miss.scroll_unmarked",
                ScrollRefusal::UnmarkedLayer => "frame_miss.scroll_unmarkedlayer",
                ScrollRefusal::RetainedLayer => "frame_miss.scroll_retainedlayer",
                ScrollRefusal::LayerReservoir => "frame_miss.scroll_layerreservoir",
                ScrollRefusal::Transform => "frame_miss.scroll_transform",
                ScrollRefusal::Fractional => "frame_miss.scroll_fractional",
            },
        }
    }

    /// This reason, recorded in the `why` journal at the caller (the check that decided it).
    #[track_caller]
    pub(in crate::document::retained) fn noted(self) -> Self {
        crate::why::note(self.code(), String::new);
        self
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
        // The DevTools codes are the counter names: one protocol vocabulary.
        let all = PLAIN
            .iter()
            .map(|(m, _)| *m)
            .chain(SCROLL.iter().map(|r| FrameMiss::Scroll(*r)));
        for (at, reason) in all.enumerate() {
            assert_eq!(reason.code(), format!("frame_miss.{}", FrameMiss::name(at)));
        }
        assert_eq!(FrameMiss::name(PLAIN.len() + 4), "scroll_interest");
    }
}
