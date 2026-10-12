//! Plain raster grid items keep intrinsic measurement until their grid area is known.
//! CSS Grid 2 #algo-overview step 4: area dimensions are definite during layout.

use crate::dom::Element;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, Refineable, Styled, StyledImage, px, size};

mod measured_leaf;
use measured_leaf::MeasuredImage;

pub(super) fn measured(
    element: &Element,
    source: &str,
    holder: &mut gpui::Div,
) -> Option<AnyElement> {
    let style = &element.style;
    if style.parent_grid == 0
        || style.position.is_some()
        || style.gradient.is_some()
        || style.bg_image.is_some()
        || style.outline.is_some()
        || !style.shadows.is_empty()
        || !style.inset_shadows.is_empty()
        || style.filter.is_some()
        || style.mask_image.is_some()
        || style.clip_ref.is_some()
        || ![
            style.radius.tl,
            style.radius.tr,
            style.radius.br,
            style.radius.bl,
        ]
        .into_iter()
        .all(zero_length)
    {
        return None;
    }
    // Decorated content boxes retain the existing holder/painting pipeline.
    let zero = |value| matches!(value, None | Some(Len::Px(0.0)));
    let border = style.borders();
    if ![
        style.padding.left,
        style.padding.right,
        style.padding.top,
        style.padding.bottom,
        border.left,
        border.right,
        border.top,
        border.bottom,
    ]
    .into_iter()
    .all(zero)
    {
        return None;
    }
    let source = crate::paint::background::source(&crate::paint::background::key(source, style))?;
    let natural = source.intrinsic();
    let crate::paint::background::Source::Raster(ready) = source else {
        return None;
    };
    let ratio = style
        .aspect_ratio
        .or(natural.ratio)
        .or_else(|| Some(natural.w? / natural.h?.max(f32::MIN_POSITIVE)));
    let mut image = gpui::img(ready)
        .preserve_natural_pixels(true)
        .pixelated(style.image_pixelated == Some(true));
    *image.style() = holder.style().clone();
    let mut native = gpui::Style::default();
    native.refine(image.style());
    native.aspect_ratio = ratio;
    // CSS Align 3 #align-grid/#justify-grid: normal on a replaced item with
    // a natural ratio is start; explicit stretch still uses the area size.
    let (mut align_start, mut justify_start) = (
        style.align_self.is_none() || style.align_self_normal,
        style.justify_self.is_none() || style.justify_self_normal,
    );
    if style.parent_grid >= 2 {
        std::mem::swap(&mut align_start, &mut justify_start);
    }
    if align_start {
        native.align_self = Some(gpui::AlignSelf::Start);
    }
    if justify_start {
        native.justify_self = Some(gpui::AlignSelf::Start);
    }
    image.style().margin = Default::default();
    image.style().min_size = Default::default();
    image.style().max_size = Default::default();
    image.style().aspect_ratio = None;
    image = image.size_full().object_fit(gpui::ObjectFit::Fill);
    Some(
        MeasuredImage {
            image: image.into_any_element(),
            style: native,
            natural: size(px(natural.w?), px(natural.h?)),
            ratio,
        }
        .into_any_element(),
    )
}

fn zero_length(value: Option<Len>) -> bool {
    matches!(value, None | Some(Len::Px(0.0)))
}

/// The measured item must see the same computed axes and inherited alignment as its holder.
pub(crate) fn inherit_item_metadata(style: &mut Computed, merged: &Computed, parent: &Computed) {
    style.parent_grid = merged.parent_grid;
    if merged.parent_grid == 0 {
        return;
    }
    style.rtl = merged.rtl;
    style.cb_vertical = merged.cb_vertical;
    style.cb_vertical_rl = merged.cb_vertical_rl;
    style.cb_sideways = merged.cb_sideways;
    style.cb_rtl = merged.cb_rtl;
    if merged.align_self.is_none() {
        style.align_self = parent.align_items;
        style.align_self_last = parent.align_items_last;
        style.align_self_safe = parent.align_items_safe;
    }
    if merged.justify_self.is_none() {
        style.justify_self = parent.justify_items;
        style.justify_self_last = parent.justify_items_last;
        style.justify_self_safe = parent.justify_items_safe;
    }
}
