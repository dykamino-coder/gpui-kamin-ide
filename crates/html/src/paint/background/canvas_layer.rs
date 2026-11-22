//! Construction of the root canvas background.

use super::*;

/// Canvas tiles are sized against the root box and cover the entire canvas.
pub(crate) fn canvas_layer(
    layer: &crate::style::computed::BgLayer,
    geometry: std::rc::Rc<Computed>,
    area: RootArea,
    vcx: ValueCx<'_>,
) -> Option<AnyElement> {
    layer.image.as_ref()?;
    if layer.fixed == Some(true) {
        return Some(fixed_layer::canvas(layer, geometry, vcx));
    }
    let values = vcx.keep();
    let layer = layer.clone();
    let opaque_borders = std::array::from_fn(|i| geometry.used_border_color(i).a >= 1.0);
    Some(
        gpui::canvas(
            |_, _, _| {},
            move |clip: Bounds<Pixels>, _, window, _| {
                let vcx = values.vcx();
                let fixed = layer.fixed == Some(true);
                let bounds = if fixed { clip } else { area.rect(clip) };
                let canvas = area.canvas(fixed, clip, window);
                tiles::paint_layer_in(
                    tiles::LayerPaint {
                        layer: &layer,
                        opaque_borders,
                    },
                    &geometry,
                    bounds,
                    Some(canvas),
                    window,
                    None,
                    vcx,
                );
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .into_any_element(),
    )
}
