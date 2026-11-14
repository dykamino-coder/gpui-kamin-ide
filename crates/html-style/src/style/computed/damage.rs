//! Paint-only style damage: two computed styles that differ only in values
//! which paint reads and layout never does (Blink `StyleDifference` paint
//! invalidation, Gecko `nsChangeHint_RepaintFrame`, Servo `REPAINT`).
//!
//! The set is closed and small on purpose: `color` (inherited, so a change
//! reaches every descendant's own style) and the background colour of the box
//! itself (with g_explicit, which only presentational hints at cascade time and control painting read). Everything else, `currentcolor` values resolved into other groups
//! included, is layout damage. The destructuring is exhaustive: a new field of
//! a compared struct must be classified here.

use super::{Computed, InheritedStyle, InheritedUi, ResetBackground};

impl Computed {
    /// Whether layout reads the same values from both styles.
    pub fn same_layout(&self, other: &Self) -> bool {
        self == other || same_but_color_and(self, other, same_layout_background)
    }

    /// Whether the styles are equal but for `color`.
    pub(super) fn same_but_color(&self, other: &Self) -> bool {
        same_but_color_and(self, other, |a, b| a == b)
    }

    /// This style with the inherited UI group `ui`: what a recascade of an
    /// element whose rules changed only inherited UI values gives (Blink
    /// `kInherit` recalc, Stylo `RECASCADE_SELF`). The other groups stay shared.
    pub fn with_inherited_ui(&self, ui: super::StyleGroup<InheritedUi>) -> Computed {
        let mut out = self.clone();
        std::rc::Rc::make_mut(&mut out.inh.0).ui = ui;
        out
    }
}

/// Whether `a` and `b` are equal but for `color` and for what `background`
/// lets differ in their background groups. The destructuring is exhaustive.
fn same_but_color_and(
    a: &Computed,
    o: &Computed,
    background: fn(&ResetBackground, &ResetBackground) -> bool,
) -> bool {
    let Computed {
        decor_box,
        block_tag,
        plain_block_box,
        clone_audit: _,
        reset_text_box,
        reset_cascade,
        reset_box_layout,
        reset_flex,
        reset_border,
        reset_grid,
        reset_context,
        reset_generated,
        reset_svg_math,
        reset_positioned,
        reset_effects,
        reset_background,
        reset_mask,
        reset_shapes,
        reset_fragmentation,
        reset_transforms,
        inh,
    } = a;
    *decor_box == o.decor_box
        && *block_tag == o.block_tag
        && *plain_block_box == o.plain_block_box
        && *reset_text_box == o.reset_text_box
        && *reset_cascade == o.reset_cascade
        && *reset_box_layout == o.reset_box_layout
        && *reset_flex == o.reset_flex
        && *reset_border == o.reset_border
        && *reset_grid == o.reset_grid
        && *reset_context == o.reset_context
        && *reset_generated == o.reset_generated
        && *reset_svg_math == o.reset_svg_math
        && *reset_positioned == o.reset_positioned
        && *reset_effects == o.reset_effects
        && *reset_mask == o.reset_mask
        && *reset_shapes == o.reset_shapes
        && *reset_fragmentation == o.reset_fragmentation
        && *reset_transforms == o.reset_transforms
        && (reset_background.ptr_eq(&o.reset_background)
            || background(reset_background, &o.reset_background))
        && (inh == &o.inh || same_layout_inherited(inh, &o.inh))
}

fn same_layout_inherited(a: &InheritedStyle, b: &InheritedStyle) -> bool {
    let InheritedStyle {
        inherited_clone_audit: _,
        ui,
        font,
        text_flow,
        table_list,
        text_ink,
        writing,
    } = a;
    *font == b.font
        && *text_flow == b.text_flow
        && *table_list == b.table_list
        && *text_ink == b.text_ink
        && *writing == b.writing
        && (ui.ptr_eq(&b.ui) || same_layout_ui(ui, &b.ui))
}

fn same_layout_ui(a: &InheritedUi, b: &InheritedUi) -> bool {
    let InheritedUi {
        color: _,
        image_pixelated,
        cursor,
        hidden,
        image_orient_none,
        pointer_events_none,
        color_scheme,
        caret_color,
        accent_color,
        math_compact,
        math_shift,
        math_level,
        math_operator_form,
    } = a;
    *image_pixelated == b.image_pixelated
        && *cursor == b.cursor
        && *hidden == b.hidden
        && *image_orient_none == b.image_orient_none
        && *pointer_events_none == b.pointer_events_none
        && *color_scheme == b.color_scheme
        && *caret_color == b.caret_color
        && *accent_color == b.accent_color
        && *math_compact == b.math_compact
        && *math_shift == b.math_shift
        && *math_level == b.math_level
        && *math_operator_form == b.math_operator_form
}

fn same_layout_background(a: &ResetBackground, b: &ResetBackground) -> bool {
    let ResetBackground {
        background: _,
        bg_explicit: _,
        background_color: _,
        gradient,
        gradient_raw,
        gradient_current,
        background_blend,
        bg_fixed,
        bg_clip,
        bg_origin,
        bg_border_insets,
        bg_image,
        bg_size,
        bg_pos,
        bg_position_math,
        bg_lists,
        bg_repeat,
    } = a;
    *gradient == b.gradient
        && *gradient_raw == b.gradient_raw
        && *gradient_current == b.gradient_current
        && *background_blend == b.background_blend
        && *bg_fixed == b.bg_fixed
        && *bg_clip == b.bg_clip
        && *bg_origin == b.bg_origin
        && *bg_border_insets == b.bg_border_insets
        && *bg_image == b.bg_image
        && *bg_size == b.bg_size
        && *bg_pos == b.bg_pos
        && *bg_position_math == b.bg_position_math
        && *bg_lists == b.bg_lists
        && *bg_repeat == b.bg_repeat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::{Color, Len};

    #[test]
    fn colour_and_background_colour_are_paint_only() {
        let a = Computed::default();
        let mut b = a.clone();
        b.inh.ui.color = Some(Color {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 1.0,
        });
        b.reset_background.background = Some(Color {
            r: 0.4,
            g: 0.5,
            b: 0.6,
            a: 1.0,
        });
        assert!(a != b && a.same_layout(&b) && b.same_layout(&a));
        let mut c = b.clone();
        c.reset_box_layout.margin.top = Some(Len::Px(1.0));
        assert!(!b.same_layout(&c), "margin is layout");
        let mut d = b.clone();
        d.inh.ui.hidden = Some(true);
        assert!(!b.same_layout(&d), "visibility stays exact");
    }
}
