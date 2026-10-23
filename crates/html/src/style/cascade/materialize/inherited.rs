//! Унаследованные вычисленные данные родителя — ровно то, что читает у родителя стадия вычисленных
//! значений (`compute::compute`) у элемента без каскадного `inherit`. Их равенство — условие
//! повторного использования материализованного стиля ребёнка (Blink `ComputedStyle::InheritedEqual`).

use crate::style::computed::{BlendMode, Computed};

macro_rules! inherited_eq {
    ($($field:ident),* $(,)?) => {
        /// Совпадают ли у `a` и `b` все поля, которые `compute::compute` берёт у родителя.
        pub(super) fn inherited_eq(a: &Computed, b: &Computed) -> bool {
            $(a.$field == b.$field &&)* a.scroll.colors == b.scroll.colors
        }
    };
}

// Новое чтение `parent.<поле>` в `compute::compute` (или в функции, которую она зовёт с
// родителем) обязано попасть сюда либо за признак каскадного `inherit` (`inherits_explicitly`).
inherited_eq! {
    accent_color, auto_phrase, autospace_alpha, autospace_numeric, balance_lines,
    border_collapse, border_spacing, break_after_spaces, break_anywhere, break_anywhere_strict,
    break_word, caption_bottom, caret_color, clamp_legacy, clamp_mark, color, color_scheme,
    combine_upright,
    cursor, emphasis_auto_shape, emphasis_color, emphasis_left, emphasis_position_auto,
    emphasis_under, empty_cells_hide, flat_mode, font_adjust_base, font_alternates,
    font_families, font_family, font_features, font_kerning, font_oblique_angle,
    font_oblique_only, font_settings, font_size, font_size_adjust, font_stretch, font_synth,
    font_synth_position, font_variant_emoji, font_weight, hanging, hidden, hyphen_char,
    hyphen_limit_chars, hyphen_limit_lines, hyphenate, hyphens_auto, image_orient_none,
    image_pixelated, italic, justify_chars, keep_all, keep_spaces, lang, letter_spacing,
    line_break_loose, line_height, list_style_image, list_style_inside, list_style_type,
    math_compact, math_level, math_operator_form, math_shift, monospace, no_justify,
    nowrap, oblique, orphans, pointer_events_none, preserve_newlines, rtl, ruby_align,
    ruby_justify, ruby_merge, ruby_overhang, ruby_under, skip_ink, skip_spaces, tab_size,
    tab_size_len, text_align, text_align_last, text_box_edge_set, text_box_over, text_box_under,
    text_emphasis, text_fit, text_indent, text_indent_each_line, text_indent_hanging,
    text_orientation, text_shadow, text_shadow_rest, text_transform, text_transform_flags,
    underline_offset, underline_pos, vertical_align, webkit_box_vertical, widows,
    word_break_word, word_space_char, word_spacing, wrap_anywhere, writing_mode,
}

/// Несёт ли каскадный стиль `own` слово `inherit` у ненаследуемого свойства: такая стадия
/// читает у родителя его ненаследуемые поля, и стиль считается под каждой коробкой-родителем.
pub(super) fn inherits_explicitly(own: &Computed) -> bool {
    !own.explicit_inherit.is_empty()
        || own.inherit_bits != 0
        || own.align_inherit != 0
        || own.background_inherit
        || own.shadow_inherit
        || own.bidi_inherit
        || own.padding_inherit
        || own.width_inherit
        || own.height_inherit
        || own.background_blend == Some(BlendMode::Inherit)
        || own.snap.inherit != 0
        || own.scroll.inherit_gutter
        || own.scroll.inherit_width
        || [
            own.border_inherit_w,
            own.border_inherit_s,
            own.border_inherit_c,
            own.padding_inherit_side,
            own.margin_inherit,
            own.minmax_inherit,
            own.inset_inherit,
        ]
        .iter()
        .flatten()
        .any(|on| *on)
}
