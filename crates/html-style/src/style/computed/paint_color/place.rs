//! The formatting context, positioning, mask, shape and transform groups
//! (classes in `super`).

use super::super::{
    Appearance, ImageSource, ResetContext, ResetMask, ResetPositioned, ResetShapes, ResetTransforms,
};

pub(super) fn context(g: &ResetContext) -> bool {
    let ResetContext {
        // Read at paint: a native control paints with the colour of its box.
        appearance,
        // No colour.
        contain_value: _,
        scroller: _,
        collapsed: _,
        hug_inline: _,
        hug_claim: _,
        baseline_orthogonal: _,
        table_fixed: _,
        animation: _,
        animation_easing: _,
        transition: _,
        float_margin_offset: _,
        floated_box: _,
        clear_inherit: _,
        ortho_limit: _,
        cq_box: _,
        cq_size: _,
        orthogonal_scrollport: _,
        orthogonal_inline: _,
        ortho_col: _,
        upright_stack: _,
        lines_reversed: _,
        spacer_edge: _,
        para_vertical: _,
        logical: _,
        user_select: _,
        no_select: _,
        frame_sizing_height: _,
        wm_contained: _,
        root_box: _,
        cb_height_def_hint: _,
        quirk_pct_base_hint: _,
        float_tail: _,
        fit_arg: _,
        intrinsic_wrapper_required: _,
        contain_paint: _,
        contain_size: _,
        contain_inline_size: _,
        contain_layout: _,
        flow_root: _,
        inline_display: _,
        run_in: _,
        fragment_clip_ancestor: _,
        is_caption: _,
        row_group_kind: _,
        col_role: _,
        contain_style: _,
        contain_intrinsic: _,
        skip_content: _,
        hoisted_block: _,
        inline_control: _,
        band_kid: _,
        icb: _,
    } = g;
    matches!(appearance, Some(Appearance::Auto | Appearance::Base))
}

/// Positioning has no colour.
pub(super) fn positioned(g: &ResetPositioned) -> bool {
    let ResetPositioned {
        position: _,
        inset: _,
        anchor_name: _,
        position_anchor: _,
        implicit_anchor: _,
        position_area: _,
        position_try_fallbacks: _,
        position_try_order: _,
        position_visibility: _,
        icb_scroll_insets: _,
        viewport_unit_insets: _,
        abs_static: _,
        z_index: _,
        rel_shift: _,
        box_shift: _,
        abs_inline_level: _,
    } = g;
    false
}

pub(super) fn mask(g: &ResetMask) -> bool {
    let ResetMask {
        // Read at paint: a mask border image with colour expressions (its
        // alpha masks the box); `mask_image` is an image operand
        // (`ImageSource::has_color_expressions`).
        mask_border,
        mask_image: _,
        // No colour.
        clip_ref: _,
        clip_bare_box: _,
        clip_margin: _,
        clip_margin_box: _,
        clip_polygon: _,
        clip_polygon_evenodd: _,
        clip_round: _,
        clip_round_len: _,
        clip_shape: _,
        mask_size: _,
        mask_no_repeat: _,
        mask_repeat_list: _,
        mask_repeat_modes: _,
        mask_fit: _,
        mask_luminance: _,
        mask_alpha_mode: _,
        mask_type_alpha: _,
        mask_origin: _,
        mask_box_override: _,
        mask_user_scale: _,
        mask_clip: _,
        mask_composite: _,
        clip_rect: _,
        clip_len: _,
        clip_inset: _,
        clip_edges: _,
        clip_xywh: _,
        mask_pos: _,
        mask_pos_far: _,
        mask_pos_list: _,
        mask_border_luminance: _,
    } = g;
    mask_border
        .as_ref()
        .and_then(|border| border.src.as_ref())
        .is_some_and(ImageSource::has_color_expressions)
}

/// Shapes shape layout only; `shape_image` is an image operand.
pub(super) fn shapes(g: &ResetShapes) -> bool {
    let ResetShapes {
        object_fit: _,
        aspect_ratio: _,
        aspect_ratio_auto: _,
        shape_outside: _,
        shape_margin: _,
        shape_threshold: _,
        flow_shapes: _,
        object_position: _,
        object_view_box: _,
        shape_image: _,
    } = g;
    false
}

/// Transforms have no colour.
pub(super) fn transforms(g: &ResetTransforms) -> bool {
    let ResetTransforms {
        translate: _,
        transform: _,
        rotate_prop: _,
        scale_prop: _,
        offset_path: _,
        offset_distance: _,
        offset_rotate: _,
        offset_anchor: _,
        offset_position: _,
        offset_icb: _,
        backface_hidden: _,
        transform_origin: _,
        transform_raw: _,
        transform_ancestor: _,
        transformed_ancestor: _,
        transform_origin_raw: _,
        transform_origin_px: _,
        transform_origin_z: _,
        perspective: _,
        perspective_origin: _,
        perspective_origin_px: _,
        transform_box_fill: _,
        transform_box: _,
        perspective_frame: _,
        preserve_3d: _,
        frame_3d: _,
    } = g;
    false
}
