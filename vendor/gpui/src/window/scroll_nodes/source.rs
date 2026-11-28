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
    /// Retained paint selected by live spatial state (scrollbar end buttons).
    Visible(Rc<dyn Fn() -> bool>),
    /// A follower (`with_scroll_follower_prepaint`): its offset is `at` of
    /// the change of another node's offset since `base`, the offset that
    /// node had when the follower prepainted.
    Follow {
        on: Box<NodeSource>,
        base: Point<Pixels>,
        at: Rc<dyn Fn(Point<Pixels>) -> Point<Pixels>>,
        device_grid: bool,
    },
}

impl NodeSource {
    /// These sources paint positions on the device grid, even when their CSS
    /// representation loses low bits far down a long document.
    pub(crate) fn device_grid(&self) -> bool {
        matches!(self, Self::Handle(_) | Self::Viewport { .. } | Self::Visible(_) | Self::Follow { device_grid: true, .. })
    }

    /// The offset now at device `scale`: a handle's clamped to its scroll
    /// range, as prepaint clamps it (written back, so the handle reads what
    /// is shown), then the translation a frame paints for it, aligned to
    /// whole device pixels (`ScrollHandle::painted`), as prepaint moves by.
    pub(crate) fn offset(&self, scale: f32) -> Point<Pixels> {
        match self {
            NodeSource::Handle(handle) => {
                // DevTools `why` names a clamp that moves it (`scroll.clamped`).
                handle.clamp_to_range();
                ScrollHandle::painted(handle.offset(), handle.max_offset(), scale)
            }
            NodeSource::Viewport { .. } | NodeSource::Visible(_) => Point::default(),
            NodeSource::Anchor(offset) => offset(),
            NodeSource::Follow { on, base, at, .. } => at(on.offset(scale) - *base),
        }
    }
}

impl NodeSource {
    pub(crate) fn visible(&self) -> bool {
        match self { Self::Visible(when) => when(), _ => true }
    }

    pub(crate) fn replay_shift(
        &self,
        parent: Point<Pixels>,
        old: Point<Pixels>,
        new: Point<Pixels>,
        scale: f32,
    ) -> Point<Pixels> {
        match self {
            Self::Viewport { .. } => Point::default(),
            _ if self.device_grid() => {
                // Subtract device positions before converting back to CSS. At
                // 112000 CSS px / 1.25 DPR, (new - old) * DPR loses ~0.004 px.
                let delta = |old: Pixels, new: Pixels| px(((new.0 * scale).round() - (old.0 * scale).round()) / scale);
                parent + point(delta(old.x, new.x), delta(old.y, new.y))
            }
            _ => parent + (new - old),
        }
    }
}
