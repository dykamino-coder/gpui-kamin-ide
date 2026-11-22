//! Construction of a background layer with its own box geometry.

use super::*;

/// `layer` before it becomes an element (`decorations::Out`).
pub(crate) fn exact(
    layer: &crate::style::computed::BgLayer,
    c: &crate::style::computed::BoxStyle,
    vcx: ValueCx<'_>,
) -> Option<exact_layer::ExactLayer> {
    let values = vcx.keep();
    layer.image.as_ref()?;
    // Border paint owns a solid border-area fill. Tiles would cover the
    // entire box, so resolve the clip and image of this layer alone.
    let background = c.paint_background();
    let own = if background.layers.is_some() {
        std::borrow::Cow::Owned(background.layer_operands(layer))
    } else if background.top == *layer {
        background
    } else {
        std::borrow::Cow::Owned(crate::style::background_paint::BackgroundValues {
            top: layer.clone(),
            ..background.into_owned()
        })
    };
    if border_area_fill(&own, vcx).is_some() {
        return None;
    }
    let style = c.style_rc().clone();
    let opaque_borders = std::array::from_fn(|i| c.used_border_color(i).a >= 1.0);
    if layer.fixed == Some(true) && fixed_layer::rectangular(c, layer) {
        return Some(fixed_layer::element(layer, c, vcx));
    }
    let layer = layer.clone();
    Some(exact_layer::ExactLayer::inset(
        c.reset_background.bg_border_insets.unwrap_or([0.0; 4]),
        move |geometry: exact_layer::PaintGeometry, window: &mut gpui::Window| {
            let bounds = geometry.snapped;
            let vcx = values.vcx();
            clip::paint(&style, layer.clip, bounds, window, |window| {
                if layer.fixed == Some(true) {
                    window.paint_scroll_position_dependent();
                    // The tile is sized and positioned against the VIEWPORT
                    // but painted only inside its own box; `paint_tiles`
                    // keeps the offset between the two.
                    let view = Bounds {
                        origin: gpui::point(px(0.0), px(0.0)),
                        size: window.viewport_size(),
                    };
                    tiles::paint_layer_in(
                        tiles::LayerPaint {
                            layer: &layer,
                            opaque_borders,
                        },
                        &style,
                        view,
                        Some(bounds),
                        window,
                        Some(geometry),
                        vcx,
                    );
                } else {
                    tiles::paint_layer_in(
                        tiles::LayerPaint {
                            layer: &layer,
                            opaque_borders,
                        },
                        &style,
                        bounds,
                        None,
                        window,
                        Some(geometry),
                        vcx,
                    );
                }
            });
        },
    ))
}
