//! Область корневого фона; хранит геометрию исходной CSS-коробки.

use gpui::{Bounds, Pixels, px};

/// Слой фоновой картинки: канвас, рисующий плитки внутри своих границ.
/// Коробка позиционирования берётся из раскладки корня перед рисованием.
/// Отступы и заданные размеры остаются запасной геометрией, если замер ещё
/// недоступен: автоматическую высоту нельзя восстанавливать по высоте холста.
#[derive(Clone, Default)]
pub struct RootArea {
    /// Geometry of the real root box, recorded before the canvas is painted.
    pub(crate) measured: Option<super::root_box::Measurement>,
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
    /// Read the measured left edge of the root's padding box (`DocState::root_left`):
    /// корень `vertical-rl` без заданной ширины — по содержимому и прижат
    /// вправо, его край известен только после раскладки. Читается при
    /// ОТРИСОВКЕ: подготовка тела (`text::vertical::root_left::RecordRootLeft`) идёт раньше
    /// отрисовки холста в том же кадре.
    pub measured_left: bool,
}

impl RootArea {
    pub(crate) fn rect(
        &self,
        clip: Bounds<Pixels>,
        vcx: crate::document::state::ValueCx<'_>,
    ) -> Bounds<Pixels> {
        if let Some(bounds) = self.measured.as_ref().and_then(|area| area.get()) {
            return bounds;
        }
        let (cw, ch) = (f32::from(clip.size.width), f32::from(clip.size.height));
        let w = self.width.unwrap_or(cw - self.left - self.right).max(0.0);
        let h = self.height.unwrap_or(ch - self.top - self.bottom).max(0.0);
        let left_abs = (self.measured_left && self.width.is_none())
            .then(|| vcx.doc.root_left.get())
            .flatten()
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
