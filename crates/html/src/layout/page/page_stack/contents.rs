//! Краска содержимого листов: фрагменты, ICB-слои и fixed-слои по листам.

use super::PageStack;
use crate::layout::fragment::types::Frag;
use gpui::{App, Bounds, Pixels, Window};

impl PageStack {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_page_contents(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        s: f32,
        pages: usize,
        k: f32,
        plan: Vec<Frag>,
        rect: impl Fn(f32, f32, f32, f32) -> Bounds<Pixels>,
    ) {
        for i in 0..pages {
            let g = self.geom(i);
            if self.slot(i).is_none() || g.turn.is_multiple_of(2) {
                continue;
            }
            let (sx, sy) = self.sheet_origin(i);
            let (bx0, by0) = (
                f32::from(bounds.origin.x) * k,
                f32::from(bounds.origin.y) * k,
            );
            let (ox, oy) = (bx0 + sx * k, by0 + sy * k);
            let (w, h) = (g.size.0 * k, g.size.1 * k);
            // p -> O + R(p - O) + сдвиг, затем q -> B + s(q - B).
            let (rs, t) = if g.turn == 1 {
                // rotate-right: (x, y) -> (O.x + H - (y - O.y), O.y + (x - O.x)).
                (
                    [[0.0, -s], [s, 0.0]],
                    [
                        bx0 * (1.0 - s) + s * (ox + h + oy),
                        by0 * (1.0 - s) + s * (oy - ox),
                    ],
                )
            } else {
                // rotate-left: (x, y) -> (O.x + (y - O.y), O.y + W - (x - O.x)).
                (
                    [[0.0, s], [-s, 0.0]],
                    [
                        bx0 * (1.0 - s) + s * (ox - oy),
                        by0 * (1.0 - s) + s * (oy + w + ox),
                    ],
                )
            };
            let m = gpui::TransformationMatrix {
                rotation_scale: rs,
                translation: t,
            };
            let plan_i: Vec<Frag> = plan.iter().copied().filter(|f| f.col == i).collect();
            window.with_transformation_masked(m, |window| {
                window.paint_quad(gpui::fill(rect(sx, sy, g.size.0, g.size.1), g.bg));
                let bx = sx + g.margin[3];
                let by = sy + g.margin[0];
                let bw = (g.size.0 - g.margin[1] - g.margin[3]).max(0.0);
                let bh = (g.size.1 - g.margin[0] - g.margin[2]).max(0.0);
                if let Some(c) = g.canvas {
                    let (cx0, cy0) = (bx.max(sx), by.max(sy));
                    let cw = ((bx + bw).min(sx + g.size.0) - cx0).max(0.0);
                    let ch = ((by + bh).min(sy + g.size.1) - cy0).max(0.0);
                    window.paint_quad(gpui::fill(rect(cx0, cy0, cw, ch), c));
                }
                let (bt, bc) = g.border;
                if bt > 0.0 {
                    for r in [
                        (bx, by, bw, bt),
                        (bx, by + bh - bt, bw, bt),
                        (bx, by, bt, bh),
                        (bx + bw - bt, by, bt, bh),
                    ] {
                        window.paint_quad(gpui::fill(rect(r.0, r.1, r.2, r.3), bc));
                    }
                }
                let (ax, ay) = g.area_origin();
                let area = rect(sx + ax, sy + ay, g.area.0, g.area.1);
                for f in plan_i {
                    let mask = gpui::ContentMask {
                        bounds: rect(sx + ax, sy + ay + f.y, g.area.0, f.h).intersect(&area),
                    };
                    let kid = &mut self.kids[f.kid];
                    let el = if f.copy == 0 {
                        &mut kid.el
                    } else {
                        match kid.frags.get_mut(f.copy - 1) {
                            Some(e) => e,
                            None => continue,
                        }
                    };
                    window.with_content_mask(Some(mask), |window| el.paint(window, cx));
                }
                if let Some(layer) = self.icb.get_mut(i) {
                    for el in layer {
                        window.with_content_mask(
                            Some(gpui::ContentMask { bounds: area }),
                            |window| el.paint(window, cx),
                        );
                    }
                }
                if let Some(layer) = self.fixed.get_mut(i) {
                    for el in layer {
                        window.with_content_mask(
                            Some(gpui::ContentMask { bounds: area }),
                            |window| el.paint(window, cx),
                        );
                    }
                }
                let sheet = rect(sx, sy, g.size.0, g.size.1);
                for (p, el) in &mut self.margin_els {
                    if *p == i {
                        window.with_content_mask(
                            Some(gpui::ContentMask { bounds: sheet }),
                            |window| el.paint(window, cx),
                        );
                    }
                }
            });
        }
    }
}
