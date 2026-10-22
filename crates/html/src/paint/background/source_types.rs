//! Варианты источника фоновой картинки; общий контракт растеризации.

use crate::document::state::ValueCx;

use super::Intrinsic;
use gpui::RenderImage;
use std::sync::Arc;

/// Чем задана фоновая картинка: готовым растром или разметкой рисунка.
///
/// Рисунок нельзя раскодировать раз и навсегда: у него нет своих точек, и
/// растрировать его надо ПОД РАЗМЕР ПЛИТКИ — иначе он выходит мыльным при
/// увеличении и лишним расходом при уменьшении.
#[derive(Clone)]
pub enum Source {
    Raster(Arc<crate::resources::images::types::CpuRaster>),
    /// Filter after resolving the input image at its concrete object size.
    Filtered {
        image: Box<Source>,
        list: crate::style::computed::ImageFilter,
    },
    Vector {
        markup: String,
        size: Intrinsic,
    },
    /// Градиент: своей величины НЕТ вовсе (css-images-3 §4.4) — обе оси
    /// берутся от области, а растрируется он точно в размер плитки.
    Gradient {
        image: Arc<crate::style::computed::ImageSource>,
    },
    /// Базовая форма `clip-path` (`circle`/`ellipse`): альфа-маска буфера
    /// группы. Радиусы и центр считаются от размера плитки (= коробки).
    Shape {
        raw: String,
    },
}

impl Source {
    fn cpu(&self) -> Option<crate::resources::css_images::Source> {
        use crate::resources::css_images::Source as Cpu;
        Some(match self {
            Self::Raster(r) => Cpu::Raster(r.clone()),
            Self::Filtered { image, list } => Cpu::Filtered {
                image: Box::new(image.cpu()?),
                list: list.clone(),
            },
            Self::Vector { markup, size } => Cpu::Vector {
                markup: markup.clone(),
                size: *size,
            },
            Self::Gradient { image } => Cpu::Gradient {
                image: image.clone(),
            },
            Self::Shape { .. } => return None,
        })
    }
    pub(crate) fn from_cpu(source: crate::resources::css_images::Source) -> Self {
        use crate::resources::css_images::Source as Cpu;
        match source {
            Cpu::Raster(r) => Self::Raster(r),
            Cpu::Filtered { image, list } => Self::Filtered {
                image: Box::new(Self::from_cpu(*image)),
                list,
            },
            Cpu::Vector { markup, size } => Self::Vector { markup, size },
            Cpu::Gradient { image } => Self::Gradient { image },
        }
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
