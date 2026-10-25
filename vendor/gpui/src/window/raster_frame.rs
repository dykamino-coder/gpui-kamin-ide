//! Точная растровая привязка без выбора подложки или порядка хозяина.

use super::*;

/// Device placement for glyphs and fills, without additional rounding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RasterFrame {
    /// Logical displacement applied before glyphs are scaled to device pixels.
    pub glyph_offset: Point<Pixels>,
    /// The placement transform and its unsnapped fill counterpart.
    /// The exact transform applies only while the placement transform matches.
    pub fill_transform: Option<(TransformationMatrix, TransformationMatrix)>,
}

impl Window {
    /// Read the current device placement without interpreting host policy.
    pub fn raster_frame(&self) -> RasterFrame {
        RasterFrame {
            glyph_offset: self.glyph_offset,
            fill_transform: self.css_fill_transform,
        }
    }

    /// Apply explicit raster placement, restoring the caller also on unwind.
    pub fn with_raster_frame<R>(
        &mut self,
        mut frame: RasterFrame,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        fn swap(window: &mut Window, frame: &mut RasterFrame) {
            mem::swap(&mut window.glyph_offset, &mut frame.glyph_offset);
            mem::swap(&mut window.css_fill_transform, &mut frame.fill_transform);
        }
        swap(self, &mut frame);
        self.with_restore(frame, swap, f).0
    }
}
