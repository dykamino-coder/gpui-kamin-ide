//! The box's own text-box, background, border and effects groups
//! (classes in `super`).

use super::super::{Outline, ResetBackground, ResetBorder, ResetEffects, ResetTextBox, Shadow};
use crate::style::values::value::StyleColor;

pub(super) fn text_box(g: &ResetTextBox) -> bool {
    let ResetTextBox {
        // Read at paint: the decoration line, which also crosses spaces.
        td_color,
        td_lines,
        // Resolved before paint: decoration records and the text-shadow ink.
        decors: _,
        text_shadow_ink: _,
        // Text only: the first letter and line style text.
        first_letter: _,
        first_letter_own: _,
        first_line: _,
        first_line_own: _,
        // No colour.
        underline: _,
        line_through: _,
        text_align_match_parent: _,
        ruby_unit: _,
        ruby_suppress_breaks: _,
        text_box_trim_start: _,
        text_box_trim_end: _,
        ellipsis: _,
        overflow_marker: _,
        text_overflow_inherit: _,
        letter_spacing_after: _,
        text_shadow_raw: _,
        text_shadow_none: _,
        vertical_shift: _,
        vertical_shift_pct: _,
        vertical_shift_px: _,
        vertical_shift_len: _,
        vertical_align_text: _,
        vertical_align_base: _,
        first_letter_excluded: _,
        initial_letter: _,
        td_style: _,
        td_thickness: _,
        td_inset: _,
        ruby_role: _,
        turned_line: _,
    } = g;
    td_color.depends_on_current() && td_lines.is_some_and(|lines| lines != 0)
}

pub(super) fn background(g: &ResetBackground) -> bool {
    let ResetBackground {
        // Read at paint: a `currentcolor` background, and the authored
        // `currentcolor` gradient whose stops the cascade resolves.
        background_color,
        gradient_current,
        // Resolved before paint: the used colour and gradient; image
        // operands are classified by `ImageSource::has_color_expressions`.
        background: _,
        gradient: _,
        gradient_raw: _,
        bg_image: _,
        bg_lists: _,
        // No colour.
        bg_explicit: _,
        background_blend: _,
        bg_fixed: _,
        bg_clip: _,
        bg_origin: _,
        bg_border_insets: _,
        bg_size: _,
        bg_pos: _,
        bg_position_math: _,
        bg_repeat: _,
    } = g;
    background_color
        .as_ref()
        .is_some_and(StyleColor::depends_on_current)
        || gradient_current.is_some()
}

/// The border colours are read by `Computed::border_reads_color`, which also
/// takes those of a decoration input; this reads the outline.
pub(super) fn outline(g: &ResetBorder) -> bool {
    let ResetBorder {
        // Read at paint: the outline; the border (`border_reads_color`) with
        // the visibility and shape of its sides.
        outline,
        border_colors: _,
        border_visible: _,
        border_side_styles: _,
        border_shape: _,
        // An image operand (`ImageSource::has_color_expressions`).
        border_image: _,
        // No colour.
        border_box: _,
        border_width: _,
        radius: _,
        border_inherit: _,
        border_dashed: _,
        border_dotted: _,
        radius_ell: _,
        corner_shape: _,
    } = g;
    outline.as_ref().is_some_and(|o| {
        let Outline {
            color,
            style,
            width: _,
            offset: _,
            inset: _,
        } = o;
        // `auto` paints the accent colour of `outline-style: auto`, else
        // `currentcolor`; `None` and 0 are `none`.
        style.is_some_and(|style| style != 0)
            && color.color().is_none_or(StyleColor::depends_on_current)
    })
}

pub(super) fn effects(g: &ResetEffects) -> bool {
    let ResetEffects {
        // Read at paint: shadow colours, raw shadows resolved at the used
        // stage, and SVG filter references, resolved where they paint.
        shadows,
        inset_shadows,
        drop_shadow,
        shadow_raw,
        filter_ref,
        backdrop_ref,
        // No colour.
        opacity: _,
        shadow_inherit: _,
        backdrop_blur: _,
        backdrop_color: _,
        blend: _,
        isolate: _,
        will_change: _,
        backdrop_root_above: _,
        will_change_root: _,
        backdrop_filter_set: _,
        vt_name: _,
        filter: _,
        scrollbar_filter_ops: _,
    } = g;
    shadows
        .iter()
        .chain(inset_shadows)
        .chain(drop_shadow)
        .any(shadow_reads_color)
        || shadow_raw.is_some()
        || filter_ref.is_some()
        || backdrop_ref.is_some()
}

/// A shadow without its own colour paints `currentcolor`.
pub(super) fn shadow_reads_color(s: &Shadow) -> bool {
    let Shadow {
        color,
        x: _,
        y: _,
        blur: _,
        spread: _,
    } = s;
    color.depends_on_current()
}
