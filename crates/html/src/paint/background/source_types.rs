//! Варианты источника фоновой картинки; общий контракт растеризации.

use super::{conic, gradient_raster, rasterize_shape};
use super::{rasterize_gradient, with_viewport};

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
    Raster(Arc<RenderImage>),
    Vector {
        markup: String,
        size: Intrinsic,
    },
    /// Градиент: своей величины НЕТ вовсе (css-images-3 §4.4) — обе оси
    /// берутся от области, а растрируется он точно в размер плитки.
    Gradient {
        raw: String,
    },
    /// Базовая форма `clip-path` (`circle`/`ellipse`): альфа-маска буфера
    /// группы. Радиусы и центр считаются от размера плитки (= коробки).
    Shape {
        raw: String,
    },
}

impl Source {
    pub(crate) fn mask_raster(&self, tile: (f32, f32), scale: f32) -> Option<Arc<RenderImage>> {
        if let Source::Gradient { raw } = self
            && !raw.starts_with("cross-fade(")
            // Конический градиент и цвет-изображение растрируются своими путями
            // (`conic`, `sources::raster_color`) — им обычный путь плитки.
            && !conic::is_conic(raw)
            && crate::style::computed::parse_image_color(raw).is_none()
        {
            // CSS Masking section 7.8 uses CSS image sizing, but coverage is sampled
            // at device pixel centres. Avoid resizing a CSS-resolution alpha
            // bitmap across sharp stops at fractional window scales.
            let css = (tile.0.clamp(1.0, 2048.0), tile.1.clamp(1.0, 2048.0));
            let w = (css.0 * scale).round().clamp(1.0, 4096.0) as u32;
            let h = (css.1 * scale).round().clamp(1.0, 4096.0) as u32;
            return gradient_raster::raster(raw, w, h, css, true);
        }
        self.raster(tile)
    }

    /// Своя величина картинки.
    pub fn intrinsic(&self) -> Intrinsic {
        match self {
            Source::Raster(image) => {
                let s = image.size(0);
                let (w, h) = ((s.width.0 as f32).max(1.0), (s.height.0 as f32).max(1.0));
                Intrinsic {
                    w: Some(w),
                    h: Some(h),
                    ratio: Some(w / h),
                }
            }
            Source::Vector { size, .. } => *size,
            Source::Gradient { .. } | Source::Shape { .. } => Intrinsic {
                w: None,
                h: None,
                ratio: None,
            },
        }
    }

    /// Растр под нужный размер плитки.
    pub fn raster(&self, tile: (f32, f32)) -> Option<Arc<RenderImage>> {
        match self {
            Source::Raster(image) => Some(image.clone()),
            Source::Vector { markup, .. } => {
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
                crate::svg::raster::rasterize(&with_viewport(markup, raster), raster.0, raster.1)
            }
            Source::Gradient { raw } => {
                const LIMIT: f32 = 2048.0;
                let (w, h) = (
                    tile.0.clamp(1.0, LIMIT).round() as u32,
                    tile.1.clamp(1.0, LIMIT).round() as u32,
                );
                rasterize_gradient(raw, w, h)
            }
            Source::Shape { raw } => {
                const LIMIT: f32 = 2048.0;
                let (w, h) = (
                    tile.0.clamp(1.0, LIMIT).round() as u32,
                    tile.1.clamp(1.0, LIMIT).round() as u32,
                );
                rasterize_shape(raw, w, h, 1.0)
            }
        }
    }
}
