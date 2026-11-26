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

/// A repeated image requested around one cell's paint (`with_sprite_pattern`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PatternRequest {
    cells: (u32, u32),
    corner_radii: Corners<Pixels>,
}

impl Window {
    /// KaminIDE patch: paint the first cell of a grid of `cells` (columns,
    /// rows) equal, abutting cells of one image as ONE sprite over the grid.
    ///
    /// The image sprite `f` paints (`paint_image_with_sampling`) for the
    /// first cell is widened over the whole grid with that cell's snapped
    /// size as its pattern and `corner_radii` (the grid's) as its corners;
    /// each cell then samples exactly what its own sprite would (the shader
    /// addresses the cell's texels the same way). A background of a thousand
    /// tiles was a thousand instances. The caller guarantees that the cells
    /// step by the snapped cell size exactly (a whole number of device
    /// pixels). Returns whether a sprite took the request; when not (nothing
    /// drew, or `f` drew no image sprite), only the first cell is painted.
    pub fn with_sprite_pattern<R>(
        &mut self,
        cells: (u32, u32),
        corner_radii: Corners<Pixels>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, bool) {
        self.sprite_pattern = Some(PatternRequest {
            cells,
            corner_radii,
        });
        let result = f(self);
        (result, self.sprite_pattern.take().is_none())
    }

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
        let lease = self.image_uploads.lease(&self.sprite_atlas, &data, frame_index);
        let content_mask = self.content_mask().scale(scale_factor);
        // Unsnapped cells keep fractional edges, which only their own
        // sprites draw exactly: such a request is left untaken.
        let pattern = match sampling {
            ImageSampling::LinearSubpixel => None,
            _ => self.sprite_pattern.take(),
        };
        let corner_radii = pattern
            .map_or(corner_radii, |p| p.corner_radii)
            .scale(scale_factor);
        let opacity = self.element_opacity();
        let transformation = self.current_transformation();
        let before = self.next_frame.scene.len();
        let cell = match sampling {
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
        };
        // A pattern spans the grid of cells from the first cell's place.
        let (bounds, pattern) = match pattern {
            Some(PatternRequest { cells: (nx, ny), .. }) => (
                Bounds::new(cell.origin, size(cell.size.width * nx as f32, cell.size.height * ny as f32)),
                cell.size,
            ),
            None => (cell, Size::default()),
        };
        // Under the open scroll node like every primitive (`insert_scene_primitive`).
        self.insert_scene_primitive(PolychromeSprite {
            transformation,
            order: 0,
            pad: u32::from(matches!(
                sampling,
                ImageSampling::Nearest | ImageSampling::NearestSnapped
            )),
            grayscale: grayscale.into(),
            bounds,
            pattern,
            content_mask,
            corner_radii,
            tile,
            opacity,
        });
        if self.next_frame.scene.len() > before {
            self.next_frame.scene.keep_image(tile, lease);
            Ok(Some(tile))
        } else {
            Ok(None)
        }
    }
}
