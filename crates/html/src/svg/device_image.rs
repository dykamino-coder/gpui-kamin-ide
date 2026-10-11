//! Inline SVG keeps fractional destination geometry when drawn into the device viewport.

use gpui::{Div, ParentElement, Styled, px};

pub(super) fn element(markup: String, w: f32, h: f32) -> Option<Div> {
    // One logical pixel of canvas past the viewport keeps the content that
    // reaches beyond its edge, so the clip below can end on the snapped box
    // edge.
    const PAD: f32 = 1.0;
    let image = super::raster::rasterize(&markup, w, h)?;
    let padded = super::raster::rasterize_padded(&markup, w, h, PAD);
    let base = |v: f32| (v * super::DENSITY).round().max(1.0);
    let pad_px = (PAD * super::DENSITY).round();
    let (dw, dh) = (
        w * (base(w) + pad_px) / base(w),
        h * (base(h) + pad_px) / base(h),
    );
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
                    //
                    // The viewport CLIP, however, is the snapped box: Blink
                    // pixel-snaps the replaced content rect it clips to while
                    // the content transform stays exact, so content that
                    // overflows the viewport fills the last device row/column
                    // of a snapped box that is larger than the exact viewport
                    // (`absolute-replaced-width-023`: a 100px rect in a 50px
                    // SVG). Only such a growing box takes the padded raster,
                    // clipped per axis to the larger of both sizes; otherwise
                    // the exact raster is drawn unclipped as before.
                    let eps = px(1e-3);
                    let grows_w = bounds.size.width > px(w) + eps;
                    let grows_h = bounds.size.height > px(h) + eps;
                    match padded.clone().filter(|_| grows_w || grows_h) {
                        Some(padded) => {
                            let clip = gpui::Bounds {
                                origin: bounds.origin,
                                size: gpui::size(
                                    bounds.size.width.max(px(w)),
                                    bounds.size.height.max(px(h)),
                                ),
                            };
                            bounds.size = gpui::size(px(dw), px(dh));
                            window.with_content_mask(
                                Some(gpui::ContentMask { bounds: clip }),
                                |window| {
                                    paint(window, bounds, padded);
                                },
                            );
                        }
                        None => {
                            bounds.size = gpui::size(px(w), px(h));
                            paint(window, bounds, image.clone());
                        }
                    }
                },
            )
            .absolute()
            .size_full(),
        ),
    )
}

fn paint(
    window: &mut gpui::Window,
    bounds: gpui::Bounds<gpui::Pixels>,
    image: std::sync::Arc<gpui::RenderImage>,
) {
    let sf = window.scale_factor();
    let integral = |v: gpui::Pixels| {
        let d = f32::from(v) * sf;
        (d - d.round()).abs() < 1e-4
    };
    // SVG 2 section 8.1 retains the viewport mapping. At an integral device
    // destination, filter the source in premultiplied alpha before uploading
    // straight BGRA: interpolating straight colors darkens transparent edges.
    // Fractional destinations and transforms still require subpixel sampling.
    let filtered = (window.current_transformation() == gpui::TransformationMatrix::unit()
        && integral(bounds.origin.x)
        && integral(bounds.origin.y)
        && integral(bounds.size.width)
        && integral(bounds.size.height))
    .then(|| {
        crate::paint::background::alpha_sampling::resample(
            &image,
            (f32::from(bounds.size.width) * sf).round() as u32,
            (f32::from(bounds.size.height) * sf).round() as u32,
        )
    })
    .flatten();
    let sampling = if filtered.is_some() {
        gpui::ImageSampling::Nearest
    } else {
        gpui::ImageSampling::LinearSubpixel
    };
    let _ = window.paint_image_with_sampling(
        bounds,
        gpui::Corners::default(),
        filtered.unwrap_or(image),
        0,
        false,
        sampling,
    );
}
