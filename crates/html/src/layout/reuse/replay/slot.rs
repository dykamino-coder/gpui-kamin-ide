//! Место повтора блока в кадре и его проверка перед prepaint.

use super::super::frame::{raster_phase_matches, scaled};
use super::{ReplayRecord, plain_environment};
use gpui::{LayoutId, Pixels, Point, Window};
use std::cell::Cell;
use std::rc::Rc;

/// Место повтора в кадре: узел раскладки, если дерево кадра его отдало.
pub(in crate::layout::reuse) struct ReplaySlot {
    pub(super) record: Rc<ReplayRecord>,
    pub(super) layout: Cell<Option<LayoutId>>,
    /// How far the block lies from where it painted (`valid`): a content
    /// record whose output moves (`Ranges::movable`) replays moved.
    pub(super) offset: Cell<Point<Pixels>>,
    /// The copy meets another mask than its record's and is clipped by it
    /// (`Replay::prepaint`).
    pub(super) cut: Cell<bool>,
}

impl ReplaySlot {
    pub(in crate::layout::reuse) fn new(record: Rc<ReplayRecord>) -> Self {
        Self {
            record,
            layout: Cell::new(None),
            offset: Cell::new(Point::default()),
            cut: Cell::new(false),
        }
    }

    /// DOM node of the replayed block.
    pub(in crate::layout::reuse) fn node(&self) -> u64 {
        self.record.source.id()
    }

    /// This frame's replay is exact (`None`): its nodes are claimed, no measure
    /// was needed, the block lies where it was recorded at the same size in the
    /// same environment, and its ranges were recorded in the frame just shown;
    /// otherwise the refusal code for the `why` journal.
    pub(in crate::layout::reuse) fn refusal(&self, window: &mut Window) -> Option<&'static str> {
        let Some(id) = self.layout.get() else {
            return Some("unclaimed");
        };
        let fresh = !window.layout_stale() && plain_environment(window);
        let (origin, size) = (
            window.layout_origin_unrounded(id),
            window.layout_size_unrounded(id),
        );
        let (serial, scale, mask) = (
            window.frame_serial(),
            window.scale_factor(),
            window.content_mask(),
        );
        // A box's content (`content`, no flow key) or an inert root-flow
        // block may lie elsewhere: its movable output replays moved.
        let content = self.record.key.is_none();
        let moves = content || self.record.inert;
        let placed = self.record.ranges(|r| {
            // Its leaves keep their functions: one measured again may lie or
            // draw otherwise (`Window::retained_remeasured`).
            let remeasured = content && window.retained_remeasured(&r.layout);
            let moved = r.origin != origin;
            let refusal = match () {
                _ if remeasured => Some("remeasured"),
                _ if r.serial + 1 != serial => Some("serial"),
                _ if !r.lines.complete() => Some("lines"),
                _ if r.scale != scale => Some("scale"),
                // A clipped copy meets its clip in prepaint (`Replay`).
                _ if r.mask != mask && !r.clipped => Some("mask"),
                _ if r.size != size => Some("size"),
                _ if moved
                    && !(moves
                        && r.movable
                        && raster_phase_matches(
                            scaled(r.origin, scale),
                            scaled(origin, scale),
                        )) =>
                {
                    Some("moved")
                }
                _ => None,
            };
            if let Some(why) = refusal {
                let kind = if content { "content" } else { "block" };
                let flags = (r.movable, r.clipped, r.hovers.len());
                note_input(
                    (kind, self.node(), why),
                    (r.origin, r.size),
                    (origin, size),
                    flags,
                );
            }
            (refusal, origin - r.origin)
        });
        match placed {
            Some((None, offset)) if fresh => {
                self.offset.set(offset);
                None
            }
            Some((None, _)) => Some("stale"),
            Some((refusal, _)) => refusal,
            None => Some("ranges"),
        }
    }
}

/// Unrounded layout positions of one placement differ by float noise only;
/// their device edges round alike (`raster_phase_matches`).
fn near(a: Pixels, b: Pixels) -> bool {
    (a - b).abs() < gpui::px(1.0 / 64.0)
}

/// `inner` lies inside `outer` but for float noise (`near`).
pub(super) fn within(inner: gpui::Bounds<Pixels>, outer: gpui::Bounds<Pixels>) -> bool {
    let (a, b) = (inner, outer);
    let ge = |x: Pixels, y: Pixels| x > y || near(x, y);
    ge(a.origin.x, b.origin.x)
        && ge(a.origin.y, b.origin.y)
        && ge(b.origin.x + b.size.width, a.origin.x + a.size.width)
        && ge(b.origin.y + b.size.height, a.origin.y + a.size.height)
}

/// `a` and `b` are the same bounds but for float noise (`near`).
pub(super) fn same(a: gpui::Bounds<Pixels>, b: gpui::Bounds<Pixels>) -> bool {
    within(a, b) && within(b, a)
}

/// The `why` journal of a refused replay (`layout_retry.replay_input`):
/// which block or content (`ReplaySlot::node`), the refusal, where and how
/// large its record painted and its box lies now, and whether its output
/// moves, is clipped and holds hover boxes. Built only for a reader.
fn note_input(
    (kind, node, why): (&str, u64, &str),
    (was_at, was): (Point<Pixels>, gpui::Size<Pixels>),
    (at, size): (Point<Pixels>, gpui::Size<Pixels>),
    (movable, clipped, hovers): (bool, bool, usize),
) {
    crate::why::note("layout_retry.replay_input", || {
        format!(
            "{kind} {node:x} {why}: origin {was_at:?} -> {at:?}, size {was:?} -> {size:?}, \
             movable {movable}, clipped {clipped}, hover boxes {hovers}"
        )
    });
}

#[cfg(test)]
mod tests {
    use gpui::{point, px, size};

    #[test]
    fn a_refused_replay_names_the_input_that_differed() {
        let _reader = crate::why::test_reader();
        crate::why::take();
        let input = |w| (point(px(0.), px(10.)), size(px(w), px(20.)));
        super::note_input(
            ("content", 0x2a, "size"),
            input(100.),
            input(80.),
            (true, false, 1),
        );
        let journal = crate::why::take().journal;
        let cause = journal
            .causes
            .iter()
            .find(|c| c.code == "layout_retry.replay_input")
            .expect("no replay input in the journal");
        let detail = format!("{cause:?}");
        assert!(detail.contains("content 2a size"), "{detail}");
        assert!(detail.contains("hover boxes 1"), "{detail}");
    }
}
