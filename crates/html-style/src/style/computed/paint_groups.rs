//! The paint-only delta of a restyle as a unit: the two style groups whose
//! values `Computed::same_layout` lets differ, the inherited UI group
//! (`color`) and the box's own background.
//!
//! Blink turns the difference of two computed styles into the work of the
//! frame (`ComputedStyle::VisualInvalidationDiff`): a paint-only difference
//! invalidates paint and leaves layout (`layout_object.cc:2879`). Here the
//! restyle of a mutation batch writes such a difference into the box tree in
//! place: the box keeps every other group of its cascaded style (and its
//! identity, which a probe of the element numbers differently).

use super::Computed;

impl Computed {
    /// This style with the paint-only groups of `new`: what a recascade gives
    /// an element whose old and new styles lay out alike (`same_layout`).
    /// The other groups stay shared.
    pub fn with_paint_of(&self, new: &Computed) -> Computed {
        let mut out = if self.inh.ui.ptr_eq(&new.inh.ui) {
            self.clone()
        } else {
            self.with_inherited_ui(new.inh.ui.clone())
        };
        out.reset_background = new.reset_background.clone();
        out
    }

    /// The paint-only groups hold the same values in both styles.
    pub fn same_paint_groups(&self, other: &Computed) -> bool {
        self.inh.ui == other.inh.ui && self.reset_background == other.reset_background
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::{Color, Len};

    fn colour(r: f32) -> Option<Color> {
        Some(Color {
            r,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        })
    }

    #[test]
    fn paint_groups_of_a_style_that_lays_out_alike_give_that_style() {
        let mut old = Computed::default();
        old.reset_box_layout.margin.top = Some(Len::Px(1.0));
        let mut new = old.clone();
        assert!(old.same_paint_groups(&new));
        new.reset_background.background = colour(0.5);
        assert!(!old.same_paint_groups(&new) && old.same_layout(&new));
        assert!(old.with_paint_of(&new) == new);
        new.inh.ui.color = colour(0.25);
        assert!(old.same_layout(&new) && old.with_paint_of(&new) == new);
        // The box keeps its own identity and layout values.
        let mut boxed = old.clone();
        boxed.decor_box = 7;
        let patched = boxed.with_paint_of(&new);
        assert!(patched.decor_box == 7 && patched.same_paint_groups(&new));
        assert!(patched.reset_box_layout == old.reset_box_layout);
    }
}
