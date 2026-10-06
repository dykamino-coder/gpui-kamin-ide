//! Inline SVG keeps fractional destination geometry when drawn into the device viewport.

use gpui::{Div, ParentElement, Styled, px};

pub(super) fn element(markup: String, w: f32, h: f32) -> Option<Div> {
    let image = super::rasterize(&markup, w, h)?;
    Some(
        gpui::div().w(px(w)).h(px(h)).child(
            gpui::canvas(
                |_, _, _| {},
                move |mut bounds, _, window, _| {
                    // SVG 2 §8.1 maps user coordinates to the viewport. Rounding
                    // the destination height changes that mapping: a 150px SVG
                    // at 125% becomes 188px instead of 187.5px, stretching every
                    // child. Blink draws into dst_rect (svg_image.cc:540-558).
                    // GPUI layout_bounds rounds physical edges as well. The SVG
                    // viewport is the requested CSS size, not that rounded box.
                    bounds.size = gpui::size(px(w), px(h));
                    let _ = window.paint_image_with_sampling(
                        bounds,
                        gpui::Corners::default(),
                        image,
                        0,
                        false,
                        gpui::ImageSampling::LinearSubpixel,
                    );
                },
            )
            .absolute()
            .size_full(),
        ),
    )
}
