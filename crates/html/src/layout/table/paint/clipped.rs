//! Полоса фона ряда/группы, обрезанная прямоугольниками ячеек (CellsClipped).

use super::RowRects;
use gpui::{Bounds, IntoElement, Pixels, Window};

pub struct CellsClipped {
    pub(crate) style: crate::style::computed::Computed,
    pub(crate) rects: RowRects,
}

impl CellsClipped {
    pub fn new(rects: RowRects, style: crate::style::computed::Computed) -> Self {
        CellsClipped { style, rects }
    }

    pub(super) fn paint_band_outline(&mut self, window: &mut Window, area: Bounds<Pixels>) {
        if let Some(o) = &self.style.outline {
            let em = match self.style.font_size {
                Some(crate::style::values::value::Len::Px(v)) => v,
                _ => 16.0,
            };
            let px_of = |l: Option<crate::style::values::value::Len>| match l {
                Some(crate::style::values::value::Len::Px(v)) => v,
                Some(crate::style::values::value::Len::Em(k)) => k * em,
                _ => 0.0,
            };
            let w = px_of(o.width);
            let out = px_of(o.offset) + w;
            if let (true, true, Some(colour)) =
                (o.style != Some(0), w > 0.0, o.color.or(self.style.color))
            {
                let ring = Bounds {
                    origin: gpui::point(
                        area.origin.x - gpui::px(out),
                        area.origin.y - gpui::px(out),
                    ),
                    size: gpui::size(
                        area.size.width + gpui::px(2.0 * out),
                        area.size.height + gpui::px(2.0 * out),
                    ),
                };
                let mut quad = gpui::fill(ring, gpui::transparent_black());
                quad.border_color = colour.to_hsla();
                quad.border_widths = gpui::Edges {
                    top: gpui::px(w),
                    right: gpui::px(w),
                    bottom: gpui::px(w),
                    left: gpui::px(w),
                };
                window.paint_quad(quad);
            }
        }
    }

    pub(super) fn paint_band_shadows(&mut self, window: &mut Window, area: Bounds<Pixels>) {
        for sh in &self.style.shadows {
            let colour = if sh.color.a < 0.0 {
                self.style
                    .color
                    .unwrap_or(crate::style::values::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    })
            } else {
                sh.color
            };
            let shifted = Bounds {
                origin: gpui::point(
                    area.origin.x + gpui::px(sh.x),
                    area.origin.y + gpui::px(sh.y),
                ),
                size: area.size,
            };
            if sh.blur > 0.0 {
                // Коробка — сам охват, смещение — в тени: примитив вырезает
                // тень под СВОЕЙ коробкой (патч gpui `Shadow::box_bounds`),
                // и сдвинутый охват вырезал бы не то место.
                window.paint_drop_shadows(
                    area,
                    gpui::Corners::default(),
                    &[gpui::BoxShadow {
                        color: colour.to_hsla(),
                        offset: gpui::point(gpui::px(sh.x), gpui::px(sh.y)),
                        // σ = половина радиуса CSS (как в `apply::apply_paint`).
                        blur_radius: gpui::px(sh.blur * 0.5),
                        spread_radius: gpui::px(sh.spread),
                        inset: false,
                    }],
                );
            } else {
                let grown = Bounds {
                    origin: gpui::point(
                        shifted.origin.x - gpui::px(sh.spread),
                        shifted.origin.y - gpui::px(sh.spread),
                    ),
                    size: gpui::size(
                        shifted.size.width + gpui::px(sh.spread * 2.0),
                        shifted.size.height + gpui::px(sh.spread * 2.0),
                    ),
                };
                let mut quad = gpui::fill(grown, gpui::transparent_black());
                quad.border_color = colour.to_hsla();
                quad.border_widths = gpui::Edges {
                    top: gpui::px((sh.spread - sh.y).max(0.0)),
                    right: gpui::px((sh.spread + sh.x).max(0.0)),
                    bottom: gpui::px((sh.spread + sh.y).max(0.0)),
                    left: gpui::px((sh.spread - sh.x).max(0.0)),
                };
                window.paint_quad(quad);
            }
        }
    }
}

impl IntoElement for CellsClipped {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
