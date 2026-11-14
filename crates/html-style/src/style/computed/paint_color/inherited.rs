//! The inherited groups (classes in `super`): no inherited value is read at
//! paint by the box itself outside text, markers, controls and editable boxes.

use super::super::{
    InheritedFont, InheritedStyle, InheritedTableList, InheritedTextFlow, InheritedTextInk,
    InheritedUi, InheritedWriting,
};

pub(super) fn reads(inh: &InheritedStyle) -> bool {
    let InheritedStyle {
        inherited_clone_audit: _,
        ui,
        font,
        text_flow,
        table_list,
        text_ink,
        writing,
    } = inh;
    ui_reads(ui)
        || font_reads(font)
        || flow_reads(text_flow)
        || list_reads(table_list)
        || ink_reads(text_ink)
        || writing_reads(writing)
}

fn ui_reads(g: &InheritedUi) -> bool {
    let InheritedUi {
        // The value itself.
        color: _,
        // Read by the caret of an editable box and by controls only.
        caret_color: _,
        accent_color: _,
        // System colours are resolved by the cascade.
        color_scheme: _,
        // No colour.
        image_pixelated: _,
        cursor: _,
        hidden: _,
        image_orient_none: _,
        pointer_events_none: _,
        math_compact: _,
        math_shift: _,
        math_level: _,
        math_operator_form: _,
    } = g;
    false
}

/// Fonts have no colour.
fn font_reads(g: &InheritedFont) -> bool {
    let InheritedFont {
        font_size: _,
        font_weight: _,
        italic: _,
        oblique: _,
        font_oblique_angle: _,
        font_oblique_only: _,
        line_height: _,
        monospace: _,
        font_family: _,
        font_families: _,
        font_stretch: _,
        font_size_adjust: _,
        zoom_eff: _,
        font_features: _,
        font_settings: _,
        font_alternates: _,
        font_synth: _,
        font_kerning: _,
        font_variant_emoji: _,
        font_synth_position: _,
    } = g;
    false
}

/// Line breaking and alignment have no colour; clamp marks and hyphens are text.
fn flow_reads(g: &InheritedTextFlow) -> bool {
    let InheritedTextFlow {
        orphans: _,
        widows: _,
        text_align: _,
        text_align_last: _,
        no_justify: _,
        ruby_justify: _,
        justify_chars: _,
        text_box_over: _,
        text_box_under: _,
        text_box_edge_set: _,
        hanging: _,
        nowrap: _,
        preserve_newlines: _,
        keep_spaces: _,
        letter_spacing: _,
        clamp_mark: _,
        word_spacing: _,
        text_transform: _,
        text_transform_flags: _,
        text_indent: _,
        text_indent_each_line: _,
        text_indent_hanging: _,
        break_anywhere: _,
        break_word: _,
        balance_lines: _,
        break_anywhere_strict: _,
        line_break_loose: _,
        keep_all: _,
        auto_phrase: _,
        break_after_spaces: _,
        clamp_legacy: _,
        webkit_box_vertical: _,
        text_fit: _,
        hyphen_char: _,
        hyphens_auto: _,
        hyphen_limit_chars: _,
        hyphen_limit_lines: _,
        lang: _,
        vertical_align: _,
        wrap_anywhere: _,
        word_space_char: _,
        autospace_alpha: _,
        autospace_numeric: _,
        hyphenate: _,
        tab_size: _,
        tab_size_len: _,
        ruby_under: _,
        ruby_align: _,
        ruby_overhang: _,
        ruby_merge: _,
        word_break_word: _,
    } = g;
    false
}

/// Markers paint only on list items; tables have no colour.
fn list_reads(g: &InheritedTableList) -> bool {
    let InheritedTableList {
        list_style_type: _,
        list_style_image: _,
        border_collapse: _,
        empty_cells_hide: _,
        list_style_inside: _,
        caption_bottom: _,
        border_spacing: _,
    } = g;
    false
}

/// Text shadows and emphasis marks paint on text only (spaces get neither).
fn ink_reads(g: &InheritedTextInk) -> bool {
    let InheritedTextInk {
        text_shadow: _,
        text_shadow_rest: _,
        emphasis_color: _,
        text_emphasis: _,
        emphasis_auto_shape: _,
        emphasis_position_auto: _,
        emphasis_under: _,
        emphasis_left: _,
        underline_offset: _,
        underline_pos: _,
        skip_ink: _,
        skip_spaces: _,
    } = g;
    false
}

/// Writing modes have no colour.
fn writing_reads(g: &InheritedWriting) -> bool {
    let InheritedWriting {
        rtl: _,
        combine_upright: _,
        writing_mode: _,
        flat_mode: _,
        text_orientation: _,
    } = g;
    false
}
