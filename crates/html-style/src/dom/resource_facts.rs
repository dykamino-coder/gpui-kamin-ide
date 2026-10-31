//! Природные размеры встроенного SVG-ресурса из разметки, без CSS sizing и отрисовки.

use super::Element;

#[derive(Clone, Copy)]
pub struct SvgFacts {
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub view_box: Option<(f32, f32)>,
}

impl SvgFacts {
    pub fn of(e: &Element) -> Self {
        let number = |name| {
            e.attr(name)
                .and_then(|v| v.trim().trim_end_matches("px").parse().ok())
        };
        Self {
            width: number("width"),
            height: number("height"),
            view_box: view_box_ratio(e),
        }
    }
}

/// Природное соотношение сторон, выраженное двумя сторонами viewport.
pub fn view_box_ratio(e: &Element) -> Option<(f32, f32)> {
    e.attr("viewBox").and_then(|vb| {
        let p: Vec<f32> = vb
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        (p.len() == 4 && p[2] > 0.0 && p[3] > 0.0).then(|| (p[2], p[3]))
    })
}
