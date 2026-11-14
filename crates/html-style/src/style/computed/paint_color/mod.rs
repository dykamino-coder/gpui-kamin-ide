//! Whether the paint of a box reads its computed `color`: a style that differs
//! from another only in `color` paints the same where it does not (Blink
//! `LayoutObject::AdjustStyleDifference`, layout_object.cc:2889-2905, which
//! paint-invalidates only objects that read `currentcolor`; Gecko repaints
//! every frame whose `color` changed, nsStyleStruct.cpp:3203-3205
//! `nsStyleText::CalcDifference`, and relies on its retained display list).
//!
//! Every field of every group is classified here, group by group in the order
//! of the classifying modules; the destructuring has no `..`, so a new field
//! does not compile until it is classified:
//! - read at paint: a value kept as `currentcolor` or a function of it that
//!   paint resolves with the box's colour (`used_color`): background, border,
//!   outline, shadow, column and row rule, scrollbar and SVG paint colours,
//!   and values paint resolves late (raw shadows, filter references, image
//!   operands with colour expressions, generated content, markers, MathML,
//!   native controls); a decoration line, which crosses spaces too;
//! - resolved before paint: values derived from `color` at cascade or merge
//!   (the used background, gradient stops, decoration records, inline
//!   decoration colours) differ themselves when `color` does, so a style equal
//!   but for `color` (`same_unread_color`) has them equal;
//! - read by text only: text shadows, emphasis marks, the colours of the
//!   first letter and line, the text-shadow ink; and by controls, editable
//!   boxes and markers only: `caret-color`, `accent-color`, `list-style-*`.
//!   The tree compare takes no inked text, control, editable box or list item
//!   (`dom::same_unread_color_tree_as`);
//! - the rest: values without a colour.

mod boxes;
mod content;
mod inherited;
mod layout;
mod place;

use super::Computed;
use crate::style::values::value::StyleColor;

impl Computed {
    /// Whether the paint of this box resolves its computed `color`, apart
    /// from text, controls, editable boxes and markers (see the module).
    pub fn paint_reads_color(&self) -> bool {
        let Computed {
            decor_box: _,
            block_tag: _,
            plain_block_box: _,
            clone_audit: _,
            reset_text_box,
            reset_background,
            reset_border,
            reset_effects,
            reset_svg_math,
            reset_generated,
            reset_fragmentation,
            reset_box_layout,
            reset_cascade,
            reset_flex,
            reset_grid,
            reset_context,
            reset_positioned,
            reset_mask,
            reset_shapes,
            reset_transforms,
            inh,
        } = self;
        boxes::text_box(reset_text_box)
            || boxes::background(reset_background)
            || self.border_reads_color(&reset_border.border_colors)
            || boxes::outline(reset_border)
            || boxes::effects(reset_effects)
            || content::svg_math(reset_svg_math)
            || content::generated(reset_generated)
            || content::fragmentation(reset_fragmentation)
            || layout::box_layout(reset_box_layout)
            || layout::cascade(reset_cascade)
            || layout::flex(reset_flex)
            || layout::grid(reset_grid)
            || place::context(reset_context)
            || place::positioned(reset_positioned)
            || place::mask(reset_mask)
            || place::shapes(reset_shapes)
            || place::transforms(reset_transforms)
            || inherited::reads(inh)
            || self
                .image_operands_ref()
                .any(super::ImageSource::has_color_expressions)
    }

    /// Whether the border of this box, painted in `colors` (its own or those
    /// of a decoration input), resolves `currentcolor`: a visible side does.
    pub fn border_reads_color(&self, colors: &[StyleColor; 4]) -> bool {
        let border = &*self.reset_border;
        let visible = |side: usize| {
            border.border_visible[side] == Some(true)
                || border.border_side_styles[side].is_some_and(|style| style > 1)
        };
        (0..4).any(|side| colors[side].depends_on_current() && visible(side))
            || border.border_shape.is_some() && colors.iter().any(StyleColor::depends_on_current)
    }

    /// Whether this style paints like `other`: equal, or equal but for
    /// `color`, which the paint of this box does not read (`paint_reads_color`).
    pub fn same_unread_color(&self, other: &Self) -> bool {
        self == other || self.same_but_color(other) && !self.paint_reads_color()
    }
}

#[cfg(test)]
mod tests;
