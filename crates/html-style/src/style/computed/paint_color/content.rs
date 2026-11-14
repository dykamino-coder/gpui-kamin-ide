//! The SVG and MathML, generated content and fragmentation groups (classes
//! in `super`).

use super::super::{ResetFragmentation, ResetGenerated, ResetSvgMath};
use crate::style::values::value::StyleColor;

pub(super) fn svg_math(g: &ResetSvgMath) -> bool {
    let ResetSvgMath {
        // Read at paint: SVG paint `currentColor`, and MathML, whose rules
        // and operators paint the colour of the box.
        svg_fill,
        svg_stroke,
        mathml,
        // No colour.
        svg_no_css_box: _,
        svg_stroke_width: _,
        svg_x: _,
        svg_y: _,
        svg_non_scaling: _,
        math_layout: _,
        math_depth: _,
        math_font_size: _,
        svg_geometry: _,
        svg_path_length: _,
    } = g;
    let current = |paint: &Option<String>| {
        paint
            .as_deref()
            .is_some_and(|p| p.to_ascii_lowercase().contains("currentcolor"))
    };
    current(svg_fill) || current(svg_stroke) || *mathml
}

pub(super) fn generated(g: &ResetGenerated) -> bool {
    let ResetGenerated {
        // Read at paint: content in place of the box and its marker.
        content,
        marker_text,
        marker_layer,
        // No colour.
        content_visibility: _,
        no_marker: _,
        content_none: _,
        scroll_marker_group: _,
        scroll_target_group: _,
        content_inherit: _,
        quotes: _,
        counter_reset: _,
        counter_increment: _,
        counter_set: _,
    } = g;
    content.is_some() || marker_text.is_some() || marker_layer.is_some()
}

pub(super) fn fragmentation(g: &ResetFragmentation) -> bool {
    let ResetFragmentation {
        // Read at paint: the colours of a visible column or row rule.
        column_rule_color,
        column_rule_colors,
        column_rule_visible,
        column_rule_styles,
        column_rule_kinds,
        row_rule_color,
        row_rule_colors,
        row_rule_visible,
        row_rule_styles,
        row_rule_kinds,
        // No colour.
        line_clamp: _,
        clamp_auto: _,
        column_count: _,
        column_width: _,
        column_height: _,
        column_wrap: _,
        column_gap: _,
        in_multicol: _,
        column_fill_auto: _,
        column_span: _,
        page: _,
        break_inside_avoid: _,
        bdb_clone: _,
        break_before_force: _,
        break_after_force: _,
        break_before_avoid: _,
        break_after_avoid: _,
        margin_trim: _,
        column_rule_width: _,
        row_rule_width: _,
        column_rule_break: _,
        row_rule_break: _,
        column_rule_widths: _,
        column_rule_double: _,
        row_rule_double: _,
        row_rule_widths: _,
        column_rule_inset: _,
        row_rule_inset: _,
        column_rule_visibility: _,
        row_rule_visibility: _,
        rule_column_over_row: _,
        clamp_continue: _,
        clamp_discard: _,
    } = g;
    let column = *column_rule_visible == Some(true)
        || column_rule_styles.is_some()
        || column_rule_kinds.is_some();
    let row =
        *row_rule_visible == Some(true) || row_rule_styles.is_some() || row_rule_kinds.is_some();
    let reads = |one: &StyleColor, list: &Option<super::super::GapList<StyleColor>>| {
        one.depends_on_current()
            || list
                .as_ref()
                .is_some_and(|l| l.any(StyleColor::depends_on_current))
    };
    column && reads(column_rule_color, column_rule_colors)
        || row && reads(row_rule_color, row_rule_colors)
}
