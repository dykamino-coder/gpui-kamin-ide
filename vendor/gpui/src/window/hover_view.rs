//! KaminIDE patch: the hover a retained record painted, and the hover a
//! later frame would paint over the same hitboxes.
//!
//! A box with a hover style decides its paint from its hitbox
//! (`Interactivity::compute_style`), so a record of its paint holds while the
//! hover each such hitbox shows is the one it showed then. A host notes the
//! hitboxes of hover styles a record holds (`Window::retained_hovers`) and,
//! before it builds the next frame, compares them with the hover the window
//! shows now (`HoverView`): only a record over a hitbox the pointer entered or
//! left is built again, as Blink restyles only the elements whose hover state
//! changed and that have a style affected by it (`ComputedStyle::AffectedByHover`).

use super::*;
use smallvec::SmallVec;

/// The hover the window shows now, without the window: the hitboxes under
/// the pointer and what suppresses their hover (`Window::hover_shown`).
#[derive(Clone, Debug, Default)]
pub struct HoverView {
    /// The hover hitboxes of the last hit test, front to back.
    hovered: SmallVec<[HitboxId; 8]>,
    captured: Option<HitboxId>,
    /// An active drag or touch input: no hover shows.
    suppressed: bool,
    /// A keyboard input: only a captured hitbox shows its hover.
    keyboard: bool,
}

impl HoverView {
    /// Whether a box over hitbox `id` shows its hover style, as
    /// `Window::hover_shown` decides it.
    pub fn shown(&self, id: HitboxId) -> bool {
        !self.suppressed && (self.captured == Some(id) || (!self.keyboard && self.hovered.contains(&id)))
    }

    /// Every hover in `hovers` (from `Window::retained_hovers`) is shown now as then.
    pub fn holds(&self, hovers: &[(HitboxId, bool)]) -> bool {
        hovers.iter().all(|&(id, shown)| self.shown(id) == shown)
    }
}

impl Window {
    /// The hover the window shows now (`HoverView`).
    pub fn hover_view(&self, cx: &App) -> HoverView {
        let hit = &self.mouse_hit_test;
        HoverView {
            hovered: hit.ids.iter().take(hit.hover_hitbox_count).copied().collect(),
            captured: self.captured_hitbox,
            suppressed: cx.has_active_drag() || self.last_input_was_touch(),
            keyboard: self.last_input_was_keyboard(),
        }
    }

    /// The hitboxes of hover styles (`insert_hover_hitbox`) that `prepaint`,
    /// a range of this frame, inserted, each with whether its box shows its
    /// hover in this frame. Empty, without allocating, for a range without one.
    pub fn retained_hovers(&self, prepaint: &RetainedPrepaint, cx: &App) -> Box<[(HitboxId, bool)]> {
        let range = prepaint.range();
        let hitboxes = self.next_frame.hitboxes.get(range.start.hitboxes_index..range.end.hitboxes_index);
        let mut hovers: Vec<(HitboxId, bool)> = Vec::new();
        for hitbox in hitboxes.into_iter().flatten().filter(|hitbox| hitbox.hover) {
            // A hitbox placed in several segments is one box.
            if hovers.last().is_none_or(|(id, _)| *id != hitbox.id) {
                hovers.push((hitbox.id, self.hover_shown(hitbox.id, cx)));
            }
        }
        hovers.into_boxed_slice()
    }
}
