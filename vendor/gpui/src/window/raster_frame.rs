//! Точная геометрия и непрозрачные данные raster consumers без правил хозяина.

use super::*;

/// Явная raster-геометрия и данные хозяина без выбора подложки или заливки.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RasterFrame {
    /// Логическое смещение глифов перед масштабированием в пиксели устройства.
    pub glyph_offset: Point<Pixels>,
    /// Матрица размещения и соответствующая точная матрица заливки.
    pub fill_transform: Option<(TransformationMatrix, TransformationMatrix)>,
    /// Округлённые и точные границы рисуемого бокса.
    pub exact_bounds: Option<(Bounds<Pixels>, Bounds<Pixels>)>,
    pub(super) contexts: render_context::Snapshot,
}

impl RasterFrame {
    /// Создание геометрии без точного бокса и данных хозяина.
    pub fn new(
        glyph_offset: Point<Pixels>,
        fill_transform: Option<(TransformationMatrix, TransformationMatrix)>,
    ) -> Self {
        Self {
            glyph_offset,
            fill_transform,
            ..Self::default()
        }
    }

    /// Чтение непрозрачных raster-данных с сохранением идентичности Rc.
    pub fn context<T: std::any::Any>(&self) -> Option<Rc<T>> {
        self.contexts.get()
    }

    /// Установка данных хозяина; клоны frame сохраняют тот же Rc.
    pub fn set_context<T: std::any::Any>(&mut self, value: Option<Rc<T>>) {
        self.contexts.set(value);
    }
}

impl Window {
    /// Чтение raster-геометрии и данных без интерпретации правил хозяина.
    pub fn raster_frame(&self) -> &RasterFrame {
        &self.raster_frame
    }

    /// Чтение текущего контекста хозяина без клонирования всего frame.
    pub fn raster_context<T: std::any::Any>(&self) -> Option<Rc<T>> {
        self.raster_frame.context()
    }

    /// Scoped host data in the shared frame, retaining unrelated raster fields.
    pub fn with_raster_context<T: std::any::Any, R>(
        &mut self,
        value: Option<Rc<T>>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.raster_frame.contexts.replace(value);
        self.with_restore(previous, |window, previous| {
            window.raster_frame.contexts.set(previous.take());
        }, f).0
    }

    /// Применение raster-данных с восстановлением вызывающего при unwind.
    pub fn with_raster_frame<R>(
        &mut self,
        mut frame: RasterFrame,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        fn swap(window: &mut Window, frame: &mut RasterFrame) {
            mem::swap(&mut window.raster_frame, frame);
        }
        swap(self, &mut frame);
        self.with_restore(frame, swap, f).0
    }
}
