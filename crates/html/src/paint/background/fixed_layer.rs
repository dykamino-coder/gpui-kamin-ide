//! Separate fixed background tile coordinates from the scrolling clip.

use super::{
    exact_layer::{ExactLayer, PaintGeometry},
    tiles::{self, LayerPaint},
};
use crate::hooks::IntoHooked;
use crate::style::computed::{BgClip, BgLayer, BoxStyle, Computed, Sides};
use crate::style::values::value::Len;
use crate::values::ValueCx;
use gpui::{AnyElement, Bounds, Pixels, Window};
use std::rc::Rc;

pub(super) fn rectangular(c: &Computed, layer: &BgLayer) -> bool {
    !matches!(layer.clip, Some(BgClip::Text | BgClip::BorderArea))
        && c.reset_border.radius == Default::default()
        && c.reset_border.radius_ell.is_none()
        && c.reset_border.border_shape.is_none()
}

fn viewport(window: &Window) -> Bounds<Pixels> {
    Bounds {
        origin: Default::default(),
        size: window.viewport_size(),
    }
}

fn border(c: &Computed) -> Sides {
    c.reset_background.bg_border_insets.map_or_else(
        || c.borders(),
        |[t, r, b, l]| Sides {
            top: Some(Len::Px(t)),
            right: Some(Len::Px(r)),
            bottom: Some(Len::Px(b)),
            left: Some(Len::Px(l)),
        },
    )
}

pub(super) fn element(layer: &BgLayer, c: &BoxStyle, vcx: ValueCx<'_>) -> ExactLayer {
    let values = vcx.keep();
    let style = c.style_rc().clone();
    let clip_style = style.clone();
    let clip_values = values.clone();
    let clip_layer = layer.clone();
    let opaque_borders = std::array::from_fn(|i| c.used_border_color(i).a >= 1.0);
    // The node owns the actual clip; tile recording covers the full viewport.
    let mut tile = layer.clone();
    tile.clip = Some(BgClip::PaddingBox);
    ExactLayer::inset(
        c.reset_background.bg_border_insets.unwrap_or([0.0; 4]),
        move |_, window| {
            let view = viewport(window);
            tiles::paint_layer_in(
                LayerPaint {
                    layer: &tile,
                    opaque_borders,
                },
                &style,
                view,
                Some(view),
                window,
                None,
                values.vcx(),
            );
        },
    )
    .viewport(false, move |geometry: PaintGeometry, window| {
        let c = &clip_style;
        let vcx = clip_values.vcx();
        let family = c.inh.font.font_family.clone().unwrap_or_default();
        let font = match c.inh.font.font_size {
            Some(Len::Px(v)) => v,
            _ => crate::defaults::MEDIUM_FONT_SIZE_PX,
        };
        let px_of = |len| crate::text::metrics::spacing_px(len, &family, font, vcx);
        let base = if c.reset_background.bg_border_insets.is_some() {
            geometry.positioning(geometry.snapped)
        } else {
            geometry.snapped
        };
        let clip = super::paint_box::resolve(
            &LayerPaint {
                layer: &clip_layer,
                opaque_borders,
            },
            c,
            base,
            &border(c),
            px_of,
        );
        super::sampling::snapped_clip(clip, window)
    })
}

pub(super) fn canvas(layer: &BgLayer, geometry: Rc<Computed>, vcx: ValueCx<'_>) -> AnyElement {
    let values = vcx.keep();
    let layer = layer.clone();
    let opaque_borders = std::array::from_fn(|i| geometry.used_border_color(i).a >= 1.0);
    ExactLayer::inset([0.0; 4], move |_, window| {
        let view = viewport(window);
        tiles::paint_layer_in(
            LayerPaint {
                layer: &layer,
                opaque_borders,
            },
            &geometry,
            view,
            Some(view),
            window,
            None,
            values.vcx(),
        );
    })
    .viewport(true, |_, window| viewport(window))
    .hooked()
}
