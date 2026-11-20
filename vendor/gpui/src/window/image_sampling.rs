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
    /// Interpolate with untransformed destination edges snapped to device pixels.
    LinearSnapped,
    /// Preserve source pixels with untransformed edges snapped to device pixels.
    NearestSnapped,
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
        self.paint_image_with_sampling_tile(
            bounds,
            corner_radii,
            data,
            frame_index,
            grayscale,
            sampling,
        )
        .map(|_| ())
    }

    pub(super) fn paint_image_with_sampling_tile(
        &mut self,
        bounds: Bounds<Pixels>,
        corner_radii: Corners<Pixels>,
        data: Arc<RenderImage>,
        frame_index: usize,
        grayscale: bool,
        sampling: ImageSampling,
    ) -> Result<Option<AtlasTile>> {
        self.invalidator.debug_assert_paint();
        let scale_factor = self.scale_factor();
        let bounds = bounds.scale(scale_factor);
        let params = RenderImageParams {
            image_id: data.id,
            frame_index,
        };
        let tile = self
            .sprite_atlas
            .get_or_insert_with(params.into(), &mut || {
                Ok(Some((
                    data.size(frame_index),
                    data.upload_bytes(frame_index).ok_or_else(|| anyhow::anyhow!("image pixels unavailable"))?,
                )))
            })?
            .expect("Callback above only returns Some");
        let content_mask = self.content_mask().scale(scale_factor);
        let corner_radii = corner_radii.scale(scale_factor);
        let opacity = self.element_opacity();
        let transformation = self.current_transformation();
        let before = self.next_frame.scene.len();
        // Under the open scroll node like every primitive (`insert_scene_primitive`).
        self.insert_scene_primitive(PolychromeSprite {
            transformation,
            order: 0,
            pad: u32::from(matches!(
                sampling,
                ImageSampling::Nearest | ImageSampling::NearestSnapped
            )),
            grayscale: grayscale.into(),
            bounds: match sampling {
                ImageSampling::LinearSubpixel => bounds,
                ImageSampling::LinearSnapped | ImageSampling::NearestSnapped => {
                    Bounds::from_corners(
                        point(bounds.left().round(), bounds.top().round()),
                        point(bounds.right().round(), bounds.bottom().round()),
                    )
                }
                _ => bounds
                    .map_origin(|origin| origin.floor())
                    .map_size(|size| size.ceil()),
            },
            content_mask,
            corner_radii,
            tile,
            opacity,
        });
        Ok((self.next_frame.scene.len() > before).then_some(tile))
    }
}
