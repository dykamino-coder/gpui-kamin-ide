//! The box layout, cascade, flex and grid groups (classes in `super`).

use super::super::scroll::ScrollStyle;
use super::super::{ResetBoxLayout, ResetCascade, ResetFlex, ResetGrid};
use crate::style::values::value::StyleColor;

pub(super) fn box_layout(g: &ResetBoxLayout) -> bool {
    let ResetBoxLayout {
        // Read at paint: `scrollbar-color` keeps `currentcolor`.
        scroll,
        // No colour.
        snap: _,
        display: _,
        attr_width: _,
        attr_height: _,
        attr_sized: _,
        container_type: _,
        width: _,
        height: _,
        min_width: _,
        min_height: _,
        max_width: _,
        max_height: _,
        padding: _,
        margin: _,
        own_margin: _,
        custom_scrollbar: _,
        overflow_x: _,
        overflow_y: _,
        resize: _,
        float: _,
        clear: _,
        width_from_inline: _,
        calc_size: _,
        stretch_size: _,
        calc_size_basis: _,
        calc_size_max: _,
    } = g;
    let ScrollStyle {
        colors,
        forced: _,
        width: _,
        gutter: _,
        inherit_gutter: _,
        viewport_root: _,
        used: _,
        native: _,
        inherit_width: _,
    } = scroll;
    colors
        .as_ref()
        .and_then(Option::as_ref)
        .is_some_and(|colors| colors.iter().any(StyleColor::depends_on_current))
}

/// Cascade inputs: `color-mix()` of the parent's colour and the colour scheme
/// are resolved by the cascade, the rest has no colour.
pub(super) fn cascade(g: &ResetCascade) -> bool {
    let ResetCascade {
        color_function: _,
        dark_color_scheme: _,
        embedding_dark_scheme: _,
        color_initial: _,
        explicit_inherit: _,
        decl_order: _,
        side_order: _,
        inherit_longhands: _,
        inherit_bits: _,
        font_weight_step: _,
        font_size_step: _,
        bidi_override: _,
        bidi_inherit: _,
        bidi_isolate: _,
        bidi_embed: _,
        bidi_plaintext: _,
        zoom: _,
        zoom_pending: _,
        declaration_parse: _,
        css_wide_value: _,
        font_variant_set: _,
    } = g;
    false
}

/// Flex and box alignment have no colour.
pub(super) fn flex(g: &ResetFlex) -> bool {
    let ResetFlex {
        flex_dir: _,
        flex_wrap: _,
        flex_wrap_reverse: _,
        flex_balance: _,
        flex_line_count: _,
        flex_grow: _,
        flex_shrink: _,
        basis_content: _,
        align_items: _,
        align_self: _,
        align_self_normal: _,
        cross_stretched: _,
        flex_basis: _,
        justify_content: _,
        gap: _,
        flex_item_ratio: _,
        order: _,
        align_content: _,
        align_content_baseline: _,
        align_content_block: _,
        justify_items: _,
        justify_self_safe: _,
        justify_items_safe: _,
        align_self_safe: _,
        align_self_own_axis: _,
        align_self_flex_kw: _,
        justify_self_own_axis: _,
        justify_self_physical: _,
        align_self_last: _,
        align_self_decl: _,
        justify_self_last: _,
        align_items_last: _,
        justify_items_last: _,
        align_items_safe: _,
        justify_content_safe: _,
        align_content_safe: _,
        align_flow: _,
        justify_self: _,
        justify_self_normal: _,
        webkit_box: _,
        flex_main_def: _,
        flex_item: _,
        flex_main_vertical: _,
        flex_line_wraps: _,
        align_content_unsafe: _,
    } = g;
    false
}

/// Grid placement and tracks have no colour.
pub(super) fn grid(g: &ResetGrid) -> bool {
    let ResetGrid {
        grid_cols: _,
        grid_tracks: _,
        grid_areas: _,
        grid_area_name: _,
        grid_col_line_names: _,
        grid_row_line_names: _,
        grid_col_named: _,
        grid_row_named: _,
        grid_auto_fill_min: _,
        grid_auto_fill_tracks: _,
        grid_auto_fill_row: _,
        subgrid: _,
        subgrid_cols: _,
        subgrid_rows: _,
        auto_repeat_cols: _,
        auto_repeat_rows: _,
        auto_repeat_body_cols: _,
        auto_repeat_body_rows: _,
        lanes_tolerance: _,
        lanes_row: _,
        lanes_fill_reverse: _,
        lanes_track_reverse: _,
        lanes_dense: _,
        lanes_inline: _,
        lanes_taffy: _,
        grid_rows: _,
        grid_auto_cols: _,
        grid_auto_cols_list: _,
        grid_auto_rows_list: _,
        grid_auto_rows: _,
        grid_auto_flow: _,
        grid_col: _,
        grid_row: _,
    } = g;
    false
}
