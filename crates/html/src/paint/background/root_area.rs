//! Область корневого фона; хранит геометрию исходной CSS-коробки.

use gpui::{Bounds, Pixels, px};

/// Слой фоновой картинки: канвас, рисующий плитки внутри своих границ.
/// Коробка позиционирования берётся из раскладки корня перед рисованием.
/// Отступы и заданные размеры остаются запасной геометрией, если замер ещё
/// недоступен: автоматическую высоту нельзя восстанавливать по высоте холста.
#[derive(Clone, Default)]
pub struct RootArea {
    /// The root scroller a layer that is not `fixed` follows
    /// (`scroll_target::canvas_scrolling`); `None` without one.
    pub(crate) scroll: Option<gpui::ScrollHandle>,
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
    /// Read the measured left edge of the root's padding box (`LayoutState::root_left`):
    /// корень `vertical-rl` без заданной ширины — по содержимому и прижат
    /// вправо, его край известен только после раскладки. Читается при
    /// ОТРИСОВКЕ: подготовка тела (`text::vertical::root_left::RecordRootLeft`) идёт раньше
    /// отрисовки холста в том же кадре.
    pub(crate) measured_left: Option<std::rc::Rc<crate::layout::layout_state::LayoutState>>,
}

impl RootArea {
    /// The canvas a layer covers with its tiles, from the layer's bounds
    /// `clip` (the viewport). A layer following the root scroller moves with
    /// the document (CSS Backgrounds 3 §2.11.2), and a replayed step shows
    /// its content past the viewport of the build: it covers the canvas of
    /// the whole scroll range at this frame's offset, cut to what its node
    /// records (the content mask, the node's interest rect), as Blink paints
    /// the root background over the document rect into the scrolling
    /// contents. A fixed layer and one without a scroller cover `clip`.
    pub(crate) fn canvas(
        &self,
        fixed: bool,
        clip: Bounds<Pixels>,
        window: &gpui::Window,
    ) -> Bounds<Pixels> {
        let Some(scroll) = self.scroll.as_ref().filter(|_| !fixed) else {
            return clip;
        };
        let (offset, max) = (scroll.offset(), scroll.max_offset());
        let whole = Bounds {
            origin: clip.origin + offset,
            size: gpui::size(clip.size.width + max.x, clip.size.height + max.y),
        };
        whole.intersect(&window.content_mask().bounds)
    }

    pub(crate) fn rect(&self, clip: Bounds<Pixels>) -> Bounds<Pixels> {
        if let Some(bounds) = self.measured.as_ref().and_then(|area| area.get()) {
            return bounds;
        }
        let (cw, ch) = (f32::from(clip.size.width), f32::from(clip.size.height));
        let w = self.width.unwrap_or(cw - self.left - self.right).max(0.0);
        let h = self.height.unwrap_or(ch - self.top - self.bottom).max(0.0);
        let left_abs = self
            .measured_left
            .as_ref()
            .filter(|_| self.width.is_none())
            .and_then(|layout| layout.root_left.get())
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
