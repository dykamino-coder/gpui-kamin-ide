//! The paint-only delta of a restyle as a unit: the two style groups whose
//! values `Computed::same_layout` lets differ, the inherited UI group
//! (`color`) and the box's own background, the colours of its borders
//! (`Computed::same_layout_but_border_colors`) and the SVG paints `fill`,
//! `stroke` and `stroke-width`.
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
        if out.reset_border.border_colors != new.reset_border.border_colors {
            out.reset_border.border_colors = new.reset_border.border_colors.clone();
            out.reset_cascade.side_order.border[2] = new.reset_cascade.side_order.border[2];
        }
        let (svg, new_svg) = (&out.reset_svg_math, &new.reset_svg_math);
        if !svg.same_paint(new_svg) {
            out.reset_svg_math.svg_fill = new.reset_svg_math.svg_fill.clone();
            out.reset_svg_math.svg_stroke = new.reset_svg_math.svg_stroke.clone();
            out.reset_svg_math.svg_stroke_width = new.reset_svg_math.svg_stroke_width.clone();
        }
        out
    }

    /// This style (the cascaded stage of a box) with each group that differs
    /// between `old` and `new` taken from `new`: what a recascade gives a box
    /// whose damage is `Damage::Layout`. `None` when such a group of this
    /// style is not that of `old` (the build adjusted it: the box does not
    /// take the change in place). Only the groups a layout damage may change
    /// are compared (`damage::level`); the others stay this style's.
    pub fn with_layout_of(&self, old: &Computed, new: &Computed) -> Option<Computed> {
        let mut out = self.clone();
        macro_rules! take {
            ($($group:ident).+) => {
                if old.$($group).+ != new.$($group).+ {
                    if self.$($group).+ != old.$($group).+ {
                        return None;
                    }
                    out.$($group).+ = new.$($group).+.clone();
                }
            };
        }
        take!(reset_text_box);
        take!(reset_box_layout);
        take!(reset_flex);
        take!(reset_border);
        take!(reset_grid);
        take!(reset_background);
        take!(reset_svg_math);
        take!(reset_cascade.side_order);
        let (inh, new_inh) = (&old.inh.0, &new.inh.0);
        let font = inh.font != new_inh.font;
        let flow = inh.text_flow != new_inh.text_flow;
        let ui = inh.ui != new_inh.ui;
        if font || flow || ui {
            let own = &self.inh.0;
            if font && own.font != inh.font
                || flow && own.text_flow != inh.text_flow
                || ui && own.ui != inh.ui
            {
                return None;
            }
            let groups = std::rc::Rc::make_mut(&mut out.inh.0);
            if font {
                groups.font = new_inh.font.clone();
            }
            if flow {
                groups.text_flow = new_inh.text_flow.clone();
            }
            if ui {
                groups.ui = new_inh.ui.clone();
            }
        }
        Some(out)
    }

    /// The paint-only groups hold the same values in both styles.
    pub fn same_paint_groups(&self, other: &Computed) -> bool {
        self.inh.ui == other.inh.ui
            && self.reset_background == other.reset_background
            && self.reset_border.border_colors == other.reset_border.border_colors
            && self.reset_svg_math.same_paint(&other.reset_svg_math)
    }
}

impl super::ResetSvgMath {
    /// The SVG paints `fill`, `stroke` and `stroke-width` are equal.
    pub fn same_paint(&self, other: &Self) -> bool {
        self.svg_fill == other.svg_fill
            && self.svg_stroke == other.svg_stroke
            && self.svg_stroke_width == other.svg_stroke_width
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
