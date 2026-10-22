//! Варианты источника фоновой картинки; общий контракт растеризации.

use super::rasterize_gradient;
use super::{conic, gradient_raster};
use crate::document::state::ValueCx;
use crate::resources::images::viewport::with_viewport;

use super::Intrinsic;
use crate::resources::images::types::CpuRaster;
use std::sync::Arc;

/// Чем задана фоновая картинка: готовым растром или разметкой рисунка.
///
/// Рисунок нельзя раскодировать раз и навсегда: у него нет своих точек, и
/// растрировать его надо ПОД РАЗМЕР ПЛИТКИ — иначе он выходит мыльным при
/// увеличении и лишним расходом при уменьшении.
#[derive(Clone)]
pub(crate) enum Source {
    Raster(Arc<CpuRaster>),
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
}

impl Source {
    pub(crate) fn mask_raster(
        &self,
        tile: (f32, f32),
        scale: f32,
        vcx: ValueCx<'_>,
    ) -> Option<Arc<CpuRaster>> {
        if let Source::Gradient { image } = self {
            if let crate::style::computed::ImageSource::Conic(g) = image.as_ref() {
                return conic::rasterize(g, tile, scale, vcx);
            }
            if image.gradient().is_some() {
                let css = (tile.0.clamp(1.0, 2048.0), tile.1.clamp(1.0, 2048.0));
                let w = (css.0 * scale).round().clamp(1.0, 4096.0) as u32;
                let h = (css.1 * scale).round().clamp(1.0, 4096.0) as u32;
                return gradient_raster::raster(image, w, h, css, true, vcx);
            }
        }
        self.raster(tile, vcx)
    }

    pub(crate) fn sampled_raster(
        &self,
        tile: (f32, f32),
        pixelated: bool,
        vcx: ValueCx<'_>,
    ) -> Option<Arc<CpuRaster>> {
        if let Source::Filtered { image, list } = self {
            super::filtered_raster::raster(image, list, tile, pixelated, vcx)
        } else {
            self.raster(tile, vcx)
        }
    }

    /// Своя величина картинки.
    pub(crate) fn intrinsic(&self) -> Intrinsic {
        match self {
            Source::Raster(image) => {
                let s = image;
                let (w, h) = ((s.width as f32).max(1.0), (s.height as f32).max(1.0));
                Intrinsic {
                    w: Some(w),
                    h: Some(h),
                    ratio: Some(w / h),
                }
            }
            Source::Filtered { image, .. } => image.intrinsic(),
            Source::Vector { size, .. } => *size,
            Source::Gradient { .. } => Intrinsic {
                w: None,
                h: None,
                ratio: None,
            },
        }
    }

    /// Растр под нужный размер плитки.
    pub(crate) fn raster(&self, tile: (f32, f32), vcx: ValueCx<'_>) -> Option<Arc<CpuRaster>> {
        match self {
            Source::Raster(image) => Some(image.clone()),
            Source::Filtered { image, list } => {
                super::filtered_raster::raster(image, list, tile, false, vcx)
            }
            Source::Vector { markup, .. } => {
                if let Some(color) = super::flat_vector::color(markup, tile, vcx) {
                    return crate::resources::images::pixels::bgra(
                        1,
                        1,
                        vec![
                            (color.b * 255.0).round() as u8,
                            (color.g * 255.0).round() as u8,
                            (color.r * 255.0).round() as u8,
                            255,
                        ],
                    );
                }
                // Растр не бывает больше потолка: при `cover` с вытянутым
                // соотношением плитка выходит в тысячи точек по длинной
                // стороне, и растеризатор возвращал НИЧЕГО — страница
                // оставалась пустой (`wide--cover--*`, `tall--cover--*`).
                // Геометрия при этом не страдает: плитка рисуется своим
                // размером, теряется только плотность, а видна всё равно
                // только та её часть, что попала в коробку.
                const LIMIT: f32 = 2048.0;
                // Потолок по КАЖДОЙ оси отдельно: общий коэффициент при
                // крайнем соотношении (`cover` на плитке 96000x330) сжимал
                // короткую сторону до считанных точек, и растянутый обратно
                // растр мылил заливку в белёсость. Пропорции растра при этом
                // ломаются — но видима лишь часть плитки в коробке, а сам
                // рисунок растрируется в свою область просмотра целиком.
                let raster = (tile.0.clamp(1.0, LIMIT), tile.1.clamp(1.0, LIMIT));
                // CSS Images 3 §4.3.2: the concrete object size establishes the
                // SVG viewport. A bitmap allocation limit changes sampling
                // density, never the viewport or preserveAspectRatio mapping.
                vcx.doc.images.borrow().rasterize_svg(
                    &with_viewport(markup, tile),
                    crate::resources::images::svg_raster::RasterRequest::new(
                        raster.0, raster.1, 2.0, 0.0,
                    )
                    .without_external_resources(),
                )
            }
            Source::Gradient { image } => {
                const LIMIT: f32 = 2048.0;
                let (w, h) = (
                    tile.0.clamp(1.0, LIMIT).round() as u32,
                    tile.1.clamp(1.0, LIMIT).round() as u32,
                );
                rasterize_gradient(image, w, h, vcx)
            }
        }
    }
}
