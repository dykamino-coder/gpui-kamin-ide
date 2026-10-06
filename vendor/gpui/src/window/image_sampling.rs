//! Image sampling selected by the caller without changing the native UI default.

use super::*;

/// How a raster image is sampled between its source pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageSampling {
    /// Interpolate neighboring pixels when resizing an image.
    Linear,
    /// Interpolate while preserving fractional destination geometry.
    LinearSubpixel,
    /// Preserve source pixel boundaries without interpolation.
    Nearest,
}

impl Window {
    /// Paint an image with an explicit sampling mode during the paint phase.
    /// Panics if `frame_index` is invalid, like `paint_image`.
    pub fn paint_image_with_sampling(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        data: Arc<RenderImage>,
        frame_index: usize,
        grayscale: bool,
        sampling: ImageSampling,
    ) -> Result<()> {
        self.invalidator.debug_assert_paint();
        let scale_factor = self.scale_factor();
        let bounds = bounds.scale(scale_factor);
        let params = RenderImageParams {
            image_id: data.id,
            frame_index,
        };
        let tile = self
            .sprite_atlas
            .get_or_insert_with(&params.into(), &mut || {
                Ok(Some((
                    data.size(frame_index),
                    Cow::Borrowed(
                        data.as_bytes(frame_index)
                            .expect("It's the caller's job to pass a valid frame index"),
                    ),
                )))
            })?
            .expect("Callback above only returns Some");
        let content_mask = self.content_mask().scale(scale_factor);
        let corner_radii = corner_radii.scale(scale_factor);
        let opacity = self.element_opacity();
        let transformation = self.current_transformation();
        self.next_frame.scene.insert_primitive(PolychromeSprite {
            transformation,
            order: 0,
            pad: u32::from(sampling == ImageSampling::Nearest),
            grayscale,
            bounds: if sampling == ImageSampling::LinearSubpixel {
                bounds
            } else {
                bounds
                    .map_origin(|origin| origin.floor())
                    .map_size(|size| size.ceil())
            },
            content_mask,
            corner_radii,
            tile,
            opacity,
        });
        Ok(())
    }
}
