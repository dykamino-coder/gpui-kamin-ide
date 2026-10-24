//! Слой used-значений раскладки поверх нового вычисленного варианта элемента: только поля,
//! которые раскладка изменила относительно исходного вычисленного стиля. Used-правки
//! остаются на собственной коробке и не становятся источником наследования потомков.

use crate::style::computed::{Computed, InheritedStyle};

macro_rules! lay_used {
    ($($field:ident),* $(,)?) => {
        /// На `base` (вычисленный вариант элемента) — поля слоя `used`, которыми он
        /// отличается от материализованного вычисленного стиля `computed`.
        pub(super) fn lay_used(base: &mut Computed, used: &Computed, computed: &Computed) {
            // Полное перечисление: новое поле `Computed` без строки здесь не соберётся.
            let Computed { $($field),* } = used;
            $(if *$field != computed.$field {
                if stringify!($field) == "inh" {
                    lay_inherited(base, used, computed);
                } else {
                    base.$field = $field.clone();
                }
            })*
        }
    };
}

macro_rules! lay_inherited {
    ($($field:ident),* $(,)?) => {
        /// Наследуемые значения, которые поставила раскладка.
        fn lay_inherited(base: &mut Computed, used: &Computed, computed: &Computed) {
            let InheritedStyle { $($field),* } = &**used;
            $(if *$field != computed.$field {
                base.$field = $field.clone();
            })*
        }
    };
}

lay_used! {
    text_shadow_ink, explicit_inherit, snap, decl_order, side_order, display, flex_dir,
    flex_wrap, flex_wrap_reverse, flex_balance, flex_line_count, flex_grow, flex_shrink,
    attr_width, attr_height, attr_sized, basis_content, align_items, align_self,
    align_self_normal, cross_stretched, flex_basis, justify_content, gap, border_box, grid_cols,
    grid_tracks, grid_areas, grid_area_name, grid_col_line_names,
    grid_row_line_names, grid_col_named, grid_row_named, grid_auto_fill_min,
    grid_auto_fill_tracks, grid_auto_fill_row, subgrid, subgrid_cols, subgrid_rows,
    container_type, auto_repeat_cols, auto_repeat_rows, auto_repeat_body_cols,
    auto_repeat_body_rows, width, height, min_width, min_height, max_width, max_height, padding,
    margin, own_margin, border_width, radius, position, inset, anchor_name, position_anchor,
    implicit_anchor, position_area, position_try_fallbacks, position_try_order,
    position_visibility, icb_scroll_insets,
    viewport_unit_insets, custom_scrollbar, scroll, overflow_x,
    scroller, overflow_y, opacity, background, bg_explicit, border_inherit, inherit_longhands,
    inherit_bits, shadow_inherit, background_color, gradient, shadows, inset_shadows,
    font_weight_step, underline, line_through, text_align_match_parent, ruby_unit,
    ruby_suppress_breaks, text_box_trim_start, text_box_trim_end, border_visible, collapsed,
    clip_ref, clip_bare_box, mask_image, ellipsis, overflow_marker, text_overflow_inherit,
    no_marker, marker_text, marker_layer, content_none, object_fit, aspect_ratio,
    aspect_ratio_auto, flex_item_ratio, abs_static, hug_inline, hug_claim, baseline_orthogonal,
    order, align_content, align_content_baseline, align_content_block, justify_items,
    justify_self_safe, justify_items_safe, align_self_safe, align_self_own_axis,
    align_self_flex_kw, justify_self_own_axis, justify_self_physical, align_self_last,
    align_self_decl, justify_self_last, align_items_last,
    justify_items_last, align_items_safe, justify_content_safe, align_content_safe,
    lanes_tolerance, lanes_row, lanes_fill_reverse, lanes_track_reverse, lanes_dense,
    align_flow, lanes_inline, lanes_taffy, justify_self, justify_self_normal, grid_rows,
    grid_auto_cols, grid_auto_cols_list, grid_auto_rows_list, grid_auto_rows, grid_auto_flow,
    grid_col, grid_row, z_index, border_colors, font_size_step, border_side_styles,
    border_dashed, border_dotted, gradient_raw, gradient_current, background_blend,
    flat_text_background, flat_border_background, outline, backdrop_blur, backdrop_color,
    letter_spacing_after, bidi_override, bidi_inherit, bidi_isolate, bidi_embed, bidi_plaintext,
    line_clamp, clamp_auto, svg_fill, svg_stroke, svg_stroke_width, svg_x, svg_y, webkit_box,
    table_fixed, column_count, column_width, column_height, column_wrap, column_gap, translate,
    text_shadow_raw, text_shadow_none, animation, animation_easing, transition, resize,
    transform, rotate_prop, scale_prop, offset_path, offset_distance, offset_rotate,
    offset_anchor, offset_position, offset_icb, backface_hidden, transform_origin,
    transform_raw, shadow_raw, transform_ancestor, transformed_ancestor, transform_origin_raw,
    transform_origin_px, transform_origin_z, perspective, perspective_origin,
    perspective_origin_px, transform_box_fill, transform_box, svg_non_scaling,
    perspective_frame, preserve_3d, frame_3d, float, float_margin_offset, floated_box,
    clear_inherit, bg_fixed, clear, ortho_limit, cq_box, cq_size, orthogonal_scrollport,
    orthogonal_inline, ortho_col, upright_stack, lines_reversed, spacer_edge, vertical_shift,
    vertical_shift_pct, vertical_shift_px, vertical_shift_len, vertical_align_text,
    vertical_align_base, rel_shift, box_shift, parent_grid, parent_grid_align_items,
    parent_flex_grid, parent_lanes, parent_subgrid, para_vertical, logical, width_from_inline,
    user_select, no_select, frame_sizing_height, first_letter, first_letter_excluded,
    first_letter_own, first_line, first_line_own, initial_letter, inline_bg, inline_border,
    inline_pad, inline_radius, text_clip_fill, text_clip_raw, bg_clip, border_image, bg_origin,
    bg_border_insets, canvas_bg, wm_contained, root_box, cb_height_def_hint, quirk_pct_base_hint,
    calc_size, flex_main_def, flex_item, flex_main_vertical, float_tail, fit_arg,
    stretch_size, intrinsic_wrapper_required, clip_margin, clip_margin_box, clip_polygon,
    clip_polygon_evenodd, blend, isolate, will_change, contain_paint, contain_size,
    contain_inline_size, contain_layout, flow_root, plain_block_box, inline_display,
    abs_inline_level, run_in, fragment_clip_ancestor,
    in_multicol, backdrop_root_above, will_change_root, backdrop_filter_set, vt_name,
    backdrop_ref, is_caption, row_group_kind, col_role, contain_style, contain_intrinsic,
    skip_content, clip_round, clip_round_len, clip_shape, mask_size, mask_no_repeat,
    mask_repeat_list, mask_repeat_modes, mask_fit, mask_luminance, mask_alpha_mode,
    mask_type_alpha, mask_origin, mask_box_override, hoisted_block, mask_user_scale, mask_clip,
    mask_composite, clip_rect, clip_len, clip_inset, clip_edges, clip_xywh, column_fill_auto,
    scroll_marker_group, scroll_target_group, column_span, page, break_inside_avoid, bdb_clone,
    break_before_force, break_after_force, break_before_avoid, break_after_avoid, margin_trim,
    zoom, zoom_pending, column_rule_width, column_rule_visible, column_rule_color,
    row_rule_width, row_rule_visible, row_rule_color, column_rule_break, row_rule_break,
    column_rule_widths, column_rule_styles, column_rule_double, row_rule_double,
    column_rule_kinds, row_rule_kinds, column_rule_colors, row_rule_widths, row_rule_styles,
    row_rule_colors, column_rule_inset, row_rule_inset, column_rule_visibility,
    row_rule_visibility, rule_column_over_row, shape_outside, shape_margin, shape_threshold,
    flow_shapes, mask_pos, mask_pos_far, mask_pos_list, radius_ell, corner_shape, border_shape,
    filter, scrollbar_filter_ops, filter_ref, drop_shadow, bg_image, bg_size, bg_pos,
    bg_position_math, object_position, object_view_box, bg_lists, bg_repeat, content,
    content_inherit, quotes, counter_reset, counter_increment, counter_set, td_lines, td_style,
    td_color, td_thickness, td_inset, block_tag, decors, ruby_role, appearance,
    dark_color_scheme, embedding_dark_scheme, color_initial, decor_box, flex_line_wraps,
    align_content_unsafe, inline_control, band_kid, mathml, math_layout,
    math_depth, math_font_size, svg_geometry, declaration_parse, turned_line, cb_wm,
    shape_image, icb, calc_size_basis, calc_size_max, svg_path_length, css_wide_value,
    color_function, mask_border, mask_border_luminance, font_variant_set, clamp_continue, inh,
}

lay_inherited! {
    color, font_size, font_weight, italic, oblique, font_oblique_angle, font_oblique_only,
    line_height, orphans, widows, text_align, text_align_last, image_pixelated, no_justify,
    ruby_justify, justify_chars, text_box_over, text_box_under, text_box_edge_set, hanging,
    nowrap, preserve_newlines, keep_spaces, monospace, font_family, font_families,
    border_collapse, empty_cells_hide, cursor, hidden, letter_spacing, clamp_mark,
    list_style_type, list_style_inside, image_orient_none, caption_bottom, border_spacing,
    word_spacing, text_transform, text_transform_flags, text_indent, text_indent_each_line,
    text_indent_hanging, break_anywhere, break_word, balance_lines, break_anywhere_strict,
    line_break_loose, keep_all, auto_phrase, break_after_spaces, clamp_legacy,
    webkit_box_vertical, text_fit, hyphen_char, hyphens_auto, hyphen_limit_chars,
    hyphen_limit_lines, lang, vertical_align, pointer_events_none, text_shadow,
    text_shadow_rest, rtl, combine_upright, wrap_anywhere, word_space_char, autospace_alpha,
    autospace_numeric, hyphenate, color_scheme, font_stretch, font_size_adjust,
    font_adjust_base, tab_size, tab_size_len, zoom_eff, font_features,
    font_settings, font_alternates, font_synth, font_kerning, text_emphasis,
    emphasis_auto_shape, emphasis_position_auto, emphasis_under, emphasis_left, emphasis_color,
    underline_offset, underline_pos, skip_ink, skip_spaces, ruby_under, ruby_align,
    ruby_overhang, ruby_merge, caret_color, accent_color, math_compact, math_shift, math_level,
    math_operator_form, writing_mode, flat_mode, text_orientation, font_variant_emoji,
    font_synth_position, list_style_image, word_break_word,
}
