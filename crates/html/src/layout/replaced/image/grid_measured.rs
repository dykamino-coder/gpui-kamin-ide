//! Replaced flex/grid items retain intrinsic measurement until their used size is known.
//! CSS Flexbox 1 §9.2/§9.8 and Grid 2 #algo-overview supply definite dimensions.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Refineable, Styled, StyledImage, px, size};

mod automatic_minimum;

use crate::layout::replaced::measured::MeasuredReplaced;

pub(super) fn measured(
    element: &Element,
    source: &str,
    holder: &mut gpui::Div,
    vcx: ValueCx<'_>,
) -> Option<AnyElement> {
    let style = &element.style;
    // Keep the existing growing-image path: exposing its natural ratio on a
    // measured leaf also changes Taffy's cross-size calculation after flexing.
    // This adapter handles intrinsic contributions and shrinking items only.
    if (style.parent_grid == 0 && !style.flex_item)
        || (style.flex_item && style.flex_grow.is_some_and(|grow| grow > 0.0))
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
    if !style.flex_item
        && ![
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
    let source =
        crate::paint::background::source(&crate::paint::background::key(source, style, vcx), vcx)?;
    let natural = source.intrinsic();
    // Decorated vectors still need concrete-size rasterization in the existing
    // holder pipeline: the measured leaf does not transfer their content-box
    // cross size separately from padding and borders.
    if !matches!(source, crate::paint::background::Source::Raster(_))
        && (!style.flex_item
            || [
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
            .any(|edge| !zero(edge)))
    {
        return None;
    }
    // Vector images have the same intrinsic sizing contract as raster images.
    // Keep their preferred dimensions automatic until flex layout supplies its
    // definite percentage bases, rather than freezing the natural size early.
    let ready = source.raster((natural.w?, natural.h?), vcx)?;
    let ratio = style
        .aspect_ratio
        .or(natural.ratio)
        .or_else(|| Some(natural.w? / natural.h?.max(f32::MIN_POSITIVE)));
    let mut image = gpui::img(ready)
        .preserve_natural_pixels(true)
        .pixelated(style.image_pixelated == Some(true));
    if !style.flex_item {
        *image.style() = holder.style().clone();
    }
    let mut native = gpui::Style::default();
    native.refine(holder.style());
    native.aspect_ratio = ratio;
    automatic_minimum::apply(&mut native, style, natural.w?, natural.h?, ratio);
    // CSS Align 3 #align-grid/#justify-grid: normal on a replaced item with
    // a natural ratio is start; explicit stretch still uses the area size.
    let (mut align_start, mut justify_start) = (
        style.align_self.is_none() || style.align_self_normal,
        style.justify_self.is_none() || style.justify_self_normal,
    );
    if style.parent_grid >= 2 {
        std::mem::swap(&mut align_start, &mut justify_start);
    }
    if style.parent_grid != 0 && align_start {
        native.align_self = Some(gpui::AlignSelf::Start);
    }
    if style.parent_grid != 0 && justify_start {
        native.justify_self = Some(gpui::AlignSelf::Start);
    }
    image.style().margin = Default::default();
    image.style().min_size = Default::default();
    image.style().max_size = Default::default();
    image.style().aspect_ratio = None;
    image.style().padding = Default::default();
    image.style().border_widths = Default::default();
    image = image.size_full().object_fit(gpui::ObjectFit::Fill);
    // Keep decoration on the painted box, but make its dimensions follow the
    // measured flex item. An inner fixed-size image is not its min-content size:
    // the flex algorithm must be able to measure it with a known cross size.
    let picture = if style.flex_item {
        let mut painted = std::mem::replace(holder, gpui::div()).size_full();
        painted.style().margin = Default::default();
        painted.style().min_size = Default::default();
        painted.style().max_size = Default::default();
        painted.style().aspect_ratio = None;
        painted.style().content_box = Some(false);
        painted.child(image).into_any_element()
    } else {
        image.into_any_element()
    };
    Some(
        MeasuredReplaced {
            image: picture,
            style: native,
            natural: size(px(natural.w?), px(natural.h?)),
            ratio,
            measure: None,
        }
        .hooked(),
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
    style.cb_wm = merged.cb_wm;
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
