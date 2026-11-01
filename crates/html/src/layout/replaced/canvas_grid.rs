//! `<canvas>` элементом сетки: естественные размеры и пропорция холста сохраняются, пока
//! сетка разрешает циклические проценты. CSS Sizing 3 §5.2.1 разделяет внутренние вклады
//! и используемые размеры; простой холст без рамок, отбивок и эффектов отдаётся как
//! `MeasuredReplaced` с нативным стилем и выравниванием `start` по умолчанию.

use crate::defaults::DEFAULT_CANVAS_SIZE_PX;
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::style::computed::BoxStyle;
use crate::style::{computed::Computed, values::value::Len};
use gpui::{AnyElement, IntoElement, Refineable, Styled, px, size};

pub(crate) fn render(
    e: &Element,
    merged: &BoxStyle,
    parent: &Computed,
    cx: &crate::layout::cx::LayoutCx,
) -> Option<AnyElement> {
    let c = merged;
    let border = c.borders();
    if c.context.parent_grid == 0
        || c.reset_positioned.position.is_some()
        || c.contains_width()
        || c.contains_height()
        || c.reset_transforms.transform.is_some()
        || c.reset_background.gradient.is_some()
        || c.reset_background.bg_image.is_some()
        || c.reset_border.outline.is_some()
        || !c.reset_effects.shadows.is_empty()
        || !c.reset_effects.inset_shadows.is_empty()
        || c.reset_effects.filter.is_some()
        || c.reset_effects.filter_ref.is_some()
        || c.reset_mask.mask_image.is_some()
        || c.reset_mask.clip_ref.is_some()
        || [
            c.reset_box_layout.padding.left,
            c.reset_box_layout.padding.right,
            c.reset_box_layout.padding.top,
            c.reset_box_layout.padding.bottom,
            border.left,
            border.right,
            border.top,
            border.bottom,
            c.reset_border.radius.tl,
            c.reset_border.radius.tr,
            c.reset_border.radius.br,
            c.reset_border.radius.bl,
        ]
        .into_iter()
        .any(|v| !matches!(v, None | Some(Len::Px(0.0))))
    {
        return None;
    }
    let natural = |v, default| match v {
        Some(Len::Px(v)) => v,
        _ => default,
    };
    let w = natural(c.reset_box_layout.attr_width, DEFAULT_CANVAS_SIZE_PX.0);
    let ratio = c
        .reset_shapes
        .aspect_ratio
        .unwrap_or_else(|| w / natural(c.reset_box_layout.attr_height, DEFAULT_CANVAS_SIZE_PX.1));
    if !ratio.is_finite() || ratio <= 0.0 || w <= 0.0 {
        return None;
    }
    let h = w / ratio;
    let mut computed = c.clone();
    let context = computed.context.clone();
    computed.context = super::image::inherit_item_metadata(&mut computed, context, merged, parent);
    // Canvas attributes describe its bitmap, not author preferred dimensions.
    if computed.reset_box_layout.attr_sized.0 {
        computed.reset_box_layout.width = None;
    }
    if computed.reset_box_layout.attr_sized.1 {
        computed.reset_box_layout.height = None;
    }
    computed.reset_box_layout.attr_sized = (false, false);
    // Grid supplies a definite percentage base at final layout. Leave the
    // percentage in the native style for intrinsic sizing to resolve as auto.
    computed.cb_height_def = true;
    let mut painted = cx.contracts.build.styled_box(e, &computed, cx);
    let mut native = gpui::Style::default();
    native.refine(painted.style());
    native.aspect_ratio = Some(ratio);
    let (mut align_start, mut justify_start) = (
        computed.reset_flex.align_self.is_none() || computed.reset_flex.align_self_normal,
        computed.reset_flex.justify_self.is_none() || computed.reset_flex.justify_self_normal,
    );
    if c.context.parent_grid >= 2 {
        std::mem::swap(&mut align_start, &mut justify_start);
    }
    if align_start {
        native.align_self = Some(gpui::AlignSelf::Start);
    }
    if justify_start {
        native.justify_self = Some(gpui::AlignSelf::Start);
    }
    painted = painted.size_full();
    painted.style().margin = Default::default();
    painted.style().min_size = Default::default();
    painted.style().max_size = Default::default();
    painted.style().aspect_ratio = None;
    Some(
        super::measured::MeasuredReplaced {
            image: painted.into_any_element(),
            style: native,
            natural: size(px(w), px(h)),
            ratio: Some(ratio),
            measure: None,
        }
        .hooked(),
    )
}
