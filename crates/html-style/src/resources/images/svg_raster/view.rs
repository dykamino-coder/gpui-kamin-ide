//! A crop is a raster viewport, leaving the source SVG root intact.

use super::RasterRequest;
use crate::cache::FloatKey;
use resvg::tiny_skia::Transform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RasterView {
    width: FloatKey,
    height: FloatKey,
    x: FloatKey,
    y: FloatKey,
}

impl RasterRequest {
    pub fn cropped(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        let density = self.density.get();
        self.view = Some(RasterView {
            width: w.into(),
            height: h.into(),
            x: x.into(),
            y: y.into(),
        });
        self.width = (w * density).round().max(1.0) as u32;
        self.height = (h * density).round().max(1.0) as u32;
        self
    }

    pub(super) fn transform(self, width: f32, height: f32) -> Transform {
        let (w, h, x, y) = self.view.map_or((width, height, 0.0, 0.0), |view| {
            (
                view.width.get(),
                view.height.get(),
                view.x.get(),
                view.y.get(),
            )
        });
        let (sx, sy) = (self.width as f32 / w, self.height as f32 / h);
        Transform::from_row(sx, 0.0, 0.0, sy, -x * sx, -y * sy)
    }
}
