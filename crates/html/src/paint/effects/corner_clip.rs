//! Раздельные контуры декораций рамки и обрезки переполнения у коробки с `corner-shape` и
//! `overflow: clip` (CSS Borders 4 §corner-shape-rendering): декорации красятся своей группой по
//! маске `rrect`, а содержимое режется контуром, сдвинутым на опорную коробку и
//! `overflow-clip-margin`, как внутренний контур в Blink. Так декорации остаются видимыми
//! снаружи вдвинутой обрезки переполнения.

use crate::document::state::ValueCx;
use crate::hooks::IntoHooked;
use crate::style::computed::{Computed, Overflow};
use crate::style::values::value::{Len, px_lossy_zero};
use gpui::{AnyElement, App, Bounds, IntoElement, ParentElement, Pixels, Styled, Window, div, px};

pub(crate) struct CornerClip {
    pub(crate) insets: [f32; 4],
    pub(crate) decoration: AnyElement,
}

fn side(v: Option<Len>) -> f32 {
    px_lossy_zero(v)
}

/// Signed offsets from the border contour; scrolling boxes keep their own clip.
/// Explicit masks and background-clip continue through their existing compositors.
pub(crate) fn insets(c: &Computed, vcx: ValueCx<'_>) -> Option<[f32; 4]> {
    if !c.corner_shaped()
        || c.border_shape.is_some()
        || c.overflow_x != Some(Overflow::Clip)
        || c.overflow_y != Some(Overflow::Clip)
        || c.mask_image.is_some()
        || c.clip_shape.is_some()
        || c.clip_polygon.is_some()
        || c.color_clip(vcx).is_some()
    {
        return None;
    }
    let margin = c.clip_margin?;
    let b = c.borders();
    let border = [side(b.top), side(b.right), side(b.bottom), side(b.left)];
    let padding = [
        side(c.padding.top),
        side(c.padding.right),
        side(c.padding.bottom),
        side(c.padding.left),
    ];
    Some(std::array::from_fn(|i| match c.clip_margin_box {
        Some(2) => -margin,
        Some(0) => border[i] + padding[i] - margin,
        _ => border[i] - margin,
    }))
}

pub(crate) fn new(c: &Computed, mask: Option<&str>, vcx: ValueCx<'_>) -> Option<CornerClip> {
    let insets = insets(c, vcx)?;
    let mask = mask?;
    if !mask.starts_with("shape:rrect(") {
        return None;
    }
    let mut own = crate::style::computed::BoxStyle::new(c.clone());
    own.clip_margin = None;
    own.opacity = None;
    let b = c.borders();
    let layer = div()
        .relative()
        .size_full()
        .border_t(px(side(b.top)))
        .border_r(px(side(b.right)))
        .border_b(px(side(b.bottom)))
        .border_l(px(side(b.left)));
    let layer = crate::paint::apply::apply_paint(layer, &own, vcx).children(
        crate::paint::decorations::decorations(&own, false, vcx, None),
    );
    let mut decoration = super::grouped_element::Grouped::new(layer.into_any_element(), vcx);
    decoration.mask = Some(super::grouped_element::GroupMask::Geometry(
        mask.to_string(),
    ));
    Some(CornerClip {
        insets,
        decoration: decoration.hooked(),
    })
}

impl CornerClip {
    pub(crate) fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.decoration
            .layout_as_root(bounds.size.map(gpui::AvailableSpace::Definite), window, cx);
        self.decoration.prepaint_at(bounds.origin, window, cx);
    }
}

/// The expanded mask tile must contain the entire offset contour, including negative coordinates.
pub(crate) fn mask_box(
    group: &super::grouped_element::Grouped,
    bounds: Bounds<Pixels>,
) -> Bounds<Pixels> {
    let Some(clip) = &group.corner_clip else {
        return bounds;
    };
    // Match the one-pixel antialiasing guard used by contour shadow fills.
    let [t, r, b, l] = clip.insets.map(|v| (-v).max(0.0) + 1.0);
    Bounds {
        origin: gpui::point(bounds.origin.x - px(l), bounds.origin.y - px(t)),
        size: gpui::size(
            bounds.size.width + px(l + r),
            bounds.size.height + px(t + b),
        ),
    }
}
