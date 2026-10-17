//! Per-line root inline strut; CSS Pseudo 4 §first-line-styling.

use super::Paragraph;
use gpui::{Font, Pixels, Window};

#[derive(Clone)]
pub(super) struct Strut {
    font: Font,
    size: Pixels,
    height: f32,
    metrics: (f32, f32),
}

impl Paragraph {
    pub(crate) fn first_line_box(mut self, font: Font, size: Pixels, height: f32) -> Self {
        self.first_strut = Some(Strut {
            font,
            size,
            height,
            metrics: (0.0, 0.0),
        });
        self
    }

    pub(super) fn measure_first_strut(&mut self, window: &mut Window) {
        let Some(strut) = &mut self.first_strut else {
            return;
        };
        let ts = window.text_system();
        let id = ts.resolve_font(&strut.font);
        strut.metrics = crate::text::fonts::metric_overrides::extents(
            &strut.font,
            f32::from(strut.size),
            (
                f32::from(ts.ascent(id, strut.size)),
                f32::from(ts.descent(id, strut.size)).abs(),
            ),
        );
    }

    pub(super) fn root_line_box(&self, start: usize) -> (f32, (f32, f32)) {
        if start == 0
            && let Some(strut) = &self.first_strut
        {
            return (strut.height, strut.metrics);
        }
        (f32::from(self.line_height), self.strut_box)
    }
}
