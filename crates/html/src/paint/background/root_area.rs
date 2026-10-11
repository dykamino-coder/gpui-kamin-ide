//! Область корневого фона; хранит геометрию исходной CSS-коробки.

use gpui::{Bounds, Pixels, px};

/// Слой фоновой картинки: канвас, рисующий плитки внутри своих границ.
/// Коробка ПОЗИЦИОНИРОВАНИЯ корня — отступы её краёв от краёв холста.
///
/// Хранится отступами, а не готовым прямоугольником: при `width: auto` (а так
/// во всей семье `background-root-*`) размер известен только на отрисовке,
/// когда виден холст.
#[derive(Clone, Copy, Default)]
pub struct RootArea {
    /// Поле плюс рамка корня с этой стороны.
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    /// Padding-box корня, если размер ЗАДАН: размер плюс отступы по оси.
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// `vertical-rl`: заданная ширина отмеряется от ПРАВОГО края холста.
    pub from_right: bool,
    /// Ключ замера левого края padding-box корня (`interact::root_left_prev`):
    /// корень `vertical-rl` без заданной ширины — по содержимому и прижат
    /// вправо, его край известен только после раскладки. Читается при
    /// ОТРИСОВКЕ: подготовка тела (`interact::RecordRootLeft`) идёт раньше
    /// отрисовки холста в том же кадре.
    pub left_key: Option<u64>,
}

impl RootArea {
    pub(crate) fn rect(&self, clip: Bounds<Pixels>) -> Bounds<Pixels> {
        let (cw, ch) = (f32::from(clip.size.width), f32::from(clip.size.height));
        let w = self.width.unwrap_or(cw - self.left - self.right).max(0.0);
        let h = self.height.unwrap_or(ch - self.top - self.bottom).max(0.0);
        let left_abs = self
            .left_key
            .filter(|_| self.width.is_none())
            .and_then(crate::text::vertical::root_left_prev)
            .map(|l| l - f32::from(clip.origin.x));
        let (x, w) = match left_abs {
            Some(l) => (l, (cw - l - self.right).max(0.0)),
            None => (self.left_x(cw, w), w),
        };
        Bounds {
            origin: gpui::point(clip.origin.x + px(x), clip.origin.y + px(self.top)),
            size: gpui::size(px(w), px(h)),
        }
    }

    pub(crate) fn left_x(&self, cw: f32, w: f32) -> f32 {
        if self.from_right && self.width.is_some() {
            cw - self.right - w
        } else {
            self.left
        }
    }
}
