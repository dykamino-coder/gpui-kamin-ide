//! Варианты источника фоновой картинки; общий контракт растеризации.

use crate::document::state::ValueCx;

use super::Intrinsic;
use gpui::RenderImage;
use std::sync::Arc;

/// CPU-картинка и дополнительная paint-маска базовой формы.
#[derive(Clone)]
pub enum Source {
    Image(crate::resources::css_images::Source),
    /// Базовая форма `clip-path` (`circle`/`ellipse`): альфа-маска буфера
    /// группы. Радиусы и центр считаются от размера плитки (= коробки).
    Shape {
        raw: String,
    },
}

impl Source {
    fn cpu(&self) -> Option<&crate::resources::css_images::Source> {
        match self {
            Self::Image(source) => Some(source),
            Self::Shape { .. } => None,
        }
    }
    pub(crate) fn from_cpu(source: crate::resources::css_images::Source) -> Self {
        Self::Image(source)
    }
    pub fn intrinsic(&self) -> Intrinsic {
        self.cpu().map_or(
            Intrinsic {
                w: None,
                h: None,
                ratio: None,
            },
            |s| s.intrinsic(),
        )
    }
    pub(crate) fn mask_raster(
        &self,
        tile: (f32, f32),
        scale: f32,
        vcx: ValueCx<'_>,
    ) -> Option<Arc<RenderImage>> {
        if let Self::Shape { .. } = self {
            return self.raster(tile, vcx);
        }
        let cpu = self.cpu()?.mask_raster(tile, scale, vcx)?;
        super::image_adapter::upload(&cpu)
    }
    pub(crate) fn sampled_raster(
        &self,
        tile: (f32, f32),
        pixelated: bool,
        vcx: ValueCx<'_>,
    ) -> Option<Arc<RenderImage>> {
        if let Self::Shape { .. } = self {
            return self.raster(tile, vcx);
        }
        let source = self.cpu()?;
        let cpu = source.sampled_raster(tile, pixelated, vcx)?;
        super::image_adapter::upload(&cpu)
    }
    pub fn raster(&self, tile: (f32, f32), vcx: ValueCx<'_>) -> Option<Arc<RenderImage>> {
        if let Self::Shape { raw } = self {
            return super::rasterize_shape(
                raw,
                tile.0.clamp(1.0, 2048.0).round() as u32,
                tile.1.clamp(1.0, 2048.0).round() as u32,
                1.0,
                vcx,
            );
        }
        let cpu = self.cpu()?.raster(tile, vcx)?;
        super::image_adapter::upload(&cpu)
    }
}
