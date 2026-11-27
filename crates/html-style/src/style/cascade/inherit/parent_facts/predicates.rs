//! Предикаты коробки-родителя для её детей: обрезка, многоколонность, барьер `fixed`,
//! трансформ и определённость высоты содержащего блока.

use crate::style::computed::{Computed, Display as D, Overflow, Position as P};
use crate::style::values::value::Len as L;
use crate::values::ValueCx;

/// The box clips its contents: non-visible overflow, `contain: paint`, a clip path or a mask.
pub(super) fn clips(parent: &Computed) -> bool {
    let clipped = |o| matches!(o, Some(o) if o != Overflow::Visible);
    let mask = &parent.reset_mask;
    clipped(parent.reset_box_layout.overflow_x)
        || clipped(parent.reset_box_layout.overflow_y)
        || parent.reset_context.contain_paint == Some(true)
        || mask.clip_ref.is_some()
        || mask.clip_shape.is_some()
        || mask.clip_polygon.is_some()
        || mask.clip_inset.is_some()
        || mask.clip_xywh.is_some()
        || mask.clip_rect.is_some()
        || mask.clip_len.is_some()
        || mask.mask_image.is_some()
}

/// The box is a multi-column container (css-multicol-1 §2); flex and grid are not.
pub(super) fn multicol(parent: &Computed) -> bool {
    (parent.reset_fragmentation.column_count.is_some()
        || parent.reset_fragmentation.column_width.is_some()
        || parent.reset_fragmentation.column_height.is_some())
        && !matches!(
            parent.reset_box_layout.display,
            Some(D::Grid | D::InlineGrid | D::GridLanes | D::Flex | D::InlineFlex)
        )
}

/// The box is a containing block for `position: fixed` descendants: a transform,
/// css-contain-1 layout or paint containment, a filter or a promised property.
pub(super) fn fixed_barrier(parent: &Computed) -> bool {
    parent.reset_transforms.transform.is_some()
        || parent.reset_transforms.preserve_3d == Some(true)
        // css-transforms-2 §perspective-property: `perspective` other than
        // `none` is a containing block for fixed descendants too.
        || parent.reset_transforms.perspective.is_some()
        || parent.reset_context.contain_layout == Some(true)
        || parent.reset_context.contain_paint == Some(true)
        || crate::style::computed::filter_contains_fixed(parent)
        // css-will-change-1 §2.1: блок для `fixed` — и от обещанного свойства.
        || parent.reset_effects.will_change & crate::style::computed::wc::CB_FIXED != 0
}

/// Has the box a transform of its own (or a promised one)?
pub(in crate::style::cascade::inherit) fn transformed(s: &Computed) -> bool {
    s.reset_transforms.transform.is_some()
        || s.reset_transforms.translate.is_some()
        || s.reset_transforms.rotate_prop.is_some()
        || s.reset_transforms.scale_prop.is_some()
        || s.reset_effects.will_change & crate::style::computed::wc::BOX != 0
}

/// Is the height of the containing block the box gives its children definite (CSS 2.1
/// §10.5)? Definite at the root, under a laid-out or stretched parent, from an aspect
/// ratio at a definite width (css-sizing-4 §5.1), at a height in pixels, at a
/// percentage of a definite one and at an absolute box, which sizes itself; not in a
/// column flex item whose main size is indefinite (css-flexbox-1 §9.8).
pub(super) fn child_height_definite(
    parent: &crate::style::computed::BoxStyle,
    laid_out: bool,
    vcx: ValueCx<'_>,
) -> bool {
    let ratio_height = parent.reset_shapes.aspect_ratio.is_some()
        && matches!(parent.reset_box_layout.height, None | Some(L::Auto))
        && matches!(parent.reset_box_layout.width, Some(L::Px(_)));
    // Соотношение сторон само даёт главный размер (Blink
    // `AspectRatioProvidesBlockMainSize`; `flex-aspect-ratio-032/033`).
    let indefinite_column_item = parent.reset_flex.flex_main_def == Some(false)
        && !matches!(parent.reset_box_layout.height, Some(L::Px(_)))
        && !matches!(parent.reset_flex.flex_basis, Some(L::Px(_)))
        && parent.reset_shapes.aspect_ratio.is_none();
    let absolute = matches!(
        parent.reset_positioned.position,
        Some(P::Absolute) | Some(P::Fixed)
    );
    !indefinite_column_item
        && (parent.reset_context.root_box
            || laid_out
            || parent.stretched
            || ratio_height
            || match parent.reset_box_layout.height {
                Some(L::Px(_)) => true,
                // `calc(100% - 16px)` is a length-percentage: definite exactly when its
                // percentage basis is (css-values-4 §10.9, CSS 2.1 §10.5).
                Some(L::Calc(i))
                    if crate::style::values::value::calc_get(i, vcx)
                        .pct_px()
                        .is_some_and(|(pct, _)| pct == 0.0) =>
                {
                    true
                }
                Some(L::Pct(_) | L::Calc(_)) => parent.cb_height_def || absolute,
                // Доля высоты у абсолютной коробки решается всегда (§10.5: оговорка
                // «not absolutely positioned»), её высота для детей определена.
                _ => absolute,
            })
}
