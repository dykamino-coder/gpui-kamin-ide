//! Preserve canvas natural dimensions while grid resolves cyclic percentages.
//! CSS Sizing 3 §5.2.1 separates intrinsic contributions from used sizes.

use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::paint::box_tree::styled::styled_div_with;
use crate::style::{computed::Computed, values::value::Len};
use gpui::{AnyElement, IntoElement, Refineable, Styled, px, size};

pub(crate) fn render(
    e: &Element,
    merged: &Computed,
    parent: &Computed,
    cx: &crate::layout::cx::LayoutCx,
) -> Option<AnyElement> {
    let c = merged;
    let border = c.borders();
    if c.parent_grid == 0
        || c.position.is_some()
        || c.contains_width()
        || c.contains_height()
        || c.transform.is_some()
        || c.gradient.is_some()
        || c.bg_image.is_some()
        || c.outline.is_some()
        || !c.shadows.is_empty()
        || !c.inset_shadows.is_empty()
        || c.filter.is_some()
        || c.filter_ref.is_some()
        || c.mask_image.is_some()
        || c.clip_ref.is_some()
        || [
            c.padding.left,
            c.padding.right,
            c.padding.top,
            c.padding.bottom,
            border.left,
            border.right,
            border.top,
            border.bottom,
            c.radius.tl,
            c.radius.tr,
            c.radius.br,
            c.radius.bl,
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
    let w = natural(c.attr_width, 300.0);
    let ratio = c
        .aspect_ratio
        .unwrap_or_else(|| w / natural(c.attr_height, 150.0));
    if !ratio.is_finite() || ratio <= 0.0 || w <= 0.0 {
        return None;
    }
    let h = w / ratio;
    let mut computed = c.clone();
    super::image::inherit_item_metadata(&mut computed, merged, parent);
    // Canvas attributes describe its bitmap, not author preferred dimensions.
    if computed.attr_sized.0 {
        computed.width = None;
    }
    if computed.attr_sized.1 {
        computed.height = None;
    }
    computed.attr_sized = (false, false);
    // Grid supplies a definite percentage base at final layout. Leave the
    // percentage in the native style for intrinsic sizing to resolve as auto.
    computed.cb_height_def = true;
    let mut painted = styled_div_with(e, &computed, cx);
    let mut native = gpui::Style::default();
    native.refine(painted.style());
    native.aspect_ratio = Some(ratio);
    let (mut align_start, mut justify_start) = (
        computed.align_self.is_none() || computed.align_self_normal,
        computed.justify_self.is_none() || computed.justify_self_normal,
    );
    if c.parent_grid >= 2 {
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
