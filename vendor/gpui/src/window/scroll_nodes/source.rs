//! Spatial reference frames used by retained scroll replay.

use super::*;

/// The offset of a scroll node in the current state (`plan_scrolled_replay`).
#[derive(Clone)]
pub(crate) enum NodeSource {
    Handle(ScrollHandle),
    /// Stationary viewport paint with an independently moving clip.
    Viewport {
        anchored: bool,
    },
    Anchor(Rc<dyn Fn() -> Point<Pixels>>),
    /// A follower (`with_scroll_follower_prepaint`): its offset is `at` of
    /// the change of another node's offset since `base`, the offset that
    /// node had when the follower prepainted.
    Follow {
        on: Box<NodeSource>,
        base: Point<Pixels>,
        at: Rc<dyn Fn(Point<Pixels>) -> Point<Pixels>>,
    },
}

impl NodeSource {
    /// The offset now: a handle's clamped to its scroll range, as prepaint
    /// clamps it (written back, so the handle reads what is shown).
    pub(crate) fn offset(&self) -> Point<Pixels> {
        match self {
            NodeSource::Handle(handle) => {
                // DevTools `why` names a clamp that moves it (`scroll.clamped`).
                handle.clamp_to_range();
                handle.offset()
            }
            NodeSource::Viewport { .. } => Point::default(),
            NodeSource::Anchor(offset) => offset(),
            NodeSource::Follow { on, base, at } => at(on.offset() - *base),
        }
    }
}

impl NodeSource {
    pub(crate) fn replay_shift(
        &self,
        parent: Point<Pixels>,
        old: Point<Pixels>,
        new: Point<Pixels>,
    ) -> Point<Pixels> {
        match self {
            Self::Viewport { .. } => Point::default(),
            _ => parent + (new - old),
        }
    }
}
