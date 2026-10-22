//! Дети стопки листов (Kid) и предраскладка марджин-боксов.

use super::measure::measure;
use super::{PageGeom, PageStack};
use crate::layout::fragment::types::Kid;
use crate::layout::page::margin_boxes::layout_margin_boxes;
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl PageStack {
    // Margin boxes use page geometry and the current GPUI layout states.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepaint_margins(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        geoms: &[PageGeom],
        names: Vec<String>,
        pages: usize,
        tail: String,
    ) {
        if let Some(mf) = self.margin_for.clone() {
            for (p, pg) in geoms.iter().enumerate() {
                let boxes = mf(
                    p,
                    &names.get(p).cloned().unwrap_or_else(|| tail.clone()),
                    pages,
                    pg,
                );
                let (sx, sy) = self.sheet_origin(p);
                for (rect, mut el) in layout_margin_boxes(boxes, pg, window, cx) {
                    let rect = round_rect(rect);
                    el.layout_as_root(
                        size(
                            gpui::AvailableSpace::Definite(px(rect.2.max(0.0))),
                            gpui::AvailableSpace::Definite(px(rect.3.max(0.0))),
                        ),
                        window,
                        cx,
                    );
                    el.prepaint_at(
                        point(
                            bounds.origin.x + px(sx + rect.0),
                            bounds.origin.y + px(sy + rect.1),
                        ),
                        window,
                        cx,
                    );
                    self.margin_els.push((p, el));
                }
            }
        }
    }

    pub(super) fn page_kids(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        (aw, ah): (f32, f32),
    ) -> (Vec<Kid>, usize) {
        let kids = self
            .kids
            .iter_mut()
            .map(|k| measure(k, self.axis, (aw, ah), 0, window, cx))
            .collect();
        let limit = self
            .kids
            .iter()
            .map(|k| k.frags.len() + 1)
            .min()
            .unwrap_or(1);
        (kids, limit)
    }
}

/// Прямоугольник марджин-бокса, округлённый как раскладка потока (Taffy
/// `round_layout`: края — `round`, размер — разность округлённых краёв).
/// Центрированная коробка на 234.5 рисовалась с 234, а эталон, собранный
/// потоком (grid/flex), — с 235 (`margin-boxes/auto-margins-003`,
/// `overconstrained-001`).
fn round_rect((x, y, w, h): (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let (x0, y0) = (x.round(), y.round());
    (x0, y0, (x + w).round() - x0, (y + h).round() - y0)
}

#[cfg(test)]
mod tests {
    use super::round_rect;

    #[test]
    fn margin_box_rect_rounds_edges_like_taffy() {
        assert_eq!(
            round_rect((234.5, 69.5, 31.0, 31.0)),
            (235.0, 70.0, 31.0, 31.0)
        );
        // Размер — разность округлённых краёв, а не округлённая ширина.
        assert_eq!(round_rect((10.4, 0.0, 20.2, 5.0)), (10.0, 0.0, 21.0, 5.0));
    }
}
