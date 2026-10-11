//! Краска самих листов: фон, рамка и марджин-боксы (css-page-3 §painting).

use super::PageStack;
use crate::layout::fragment::types::Frag;
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl PageStack {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_sheets(
        &mut self,
        bounds: Bounds<Pixels>,
        cx: &mut App,
        s: f32,
        pages: usize,
        plan: &[Frag],
        masks: Vec<gpui::ContentMask<Pixels>>,
        rect: impl Fn(f32, f32, f32, f32) -> Bounds<Pixels>,
        window: &mut Window,
    ) {
        // Порядок краски css-page-3 §painting: фон листа → канвас
        // документа (border box листа) → рамки → содержимое.
        for i in 0..pages {
            if self.slot(i).is_none() || self.geom(i).turn % 2 == 1 {
                continue;
            }
            let (sx, sy) = self.sheet_origin(i);
            let g = self.geom(i);
            window.paint_quad(gpui::fill(rect(sx, sy, g.size.0, g.size.1), g.bg));
            let bx = sx + g.margin[3];
            let by = sy + g.margin[0];
            let bw = (g.size.0 - g.margin[1] - g.margin[3]).max(0.0);
            let bh = (g.size.1 - g.margin[0] - g.margin[2]).max(0.0);
            if let Some(c) = g.canvas {
                // Канвас кроет border box листа (§painting), но не шире
                // самого листа: при отрицательных полях жёлтый фон тела
                // вылезал полосами за правый и нижний край.
                let (cx, cy) = (bx.max(sx), by.max(sy));
                let cw = ((bx + bw).min(sx + g.size.0) - cx).max(0.0);
                let ch = ((by + bh).min(sy + g.size.1) - cy).max(0.0);
                window.paint_quad(gpui::fill(rect(cx, cy, cw, ch), c));
            }
            let (t, c) = g.border;
            if t > 0.0 {
                for r in [
                    (bx, by, bw, t),
                    (bx, by + bh - t, bw, t),
                    (bx, by, t, bh),
                    (bx + bw - t, by, t, bh),
                ] {
                    window.paint_quad(gpui::fill(rect(r.0, r.1, r.2, r.3), c));
                }
            }
            // Контур — снаружи рамки со сдвигом, в слое рамок (под
            // содержимым): `page-box-010` — поле 50, контур 10 со сдвигом
            // 40 ложится вплотную к краю листа, как рамка эталона.
            let (ow, off, oc) = g.outline;
            if ow > 0.0 {
                let (ox, oy) = (bx - off - ow, by - off - ow);
                let (fw, fh) = (bw + 2.0 * (off + ow), bh + 2.0 * (off + ow));
                for r in [
                    (ox, oy, fw, ow),
                    (ox, oy + fh - ow, fw, ow),
                    (ox, oy, ow, fh),
                    (ox + fw - ow, oy, ow, fh),
                ] {
                    window.paint_quad(gpui::fill(rect(r.0, r.1, r.2, r.3), oc));
                }
            }
        }
        // Содержимое — под маской СВОЕГО ФРАГМЕНТА: копия нарисована во
        // всю высоту, видна только полоса `[y, y + h)` этой страницы (вид
        // `slice`, css-break-4 §4; ровно как у `ColumnStack`). Маска по
        // целой page area оставляла на странице хвост следующего
        // фрагмента до края листа (`block-page-break-inside-avoid-7`).
        for f in plan.iter().copied() {
            if self.geom(f.col).turn % 2 == 1 {
                continue;
            }
            let Some(page) = masks.get(f.col).cloned() else {
                continue;
            };
            let (sx, sy) = self.sheet_origin(f.col);
            let g = self.geom(f.col);
            let (ax, ay) = g.area_origin();
            let mask = gpui::ContentMask {
                bounds: Bounds {
                    origin: point(
                        bounds.origin.x + px((sx + ax) * s),
                        bounds.origin.y + px((sy + ay + f.y) * s),
                    ),
                    size: size(px(g.area.0 * s), px(f.h * s)),
                }
                .intersect(&page.bounds),
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
            // Маски детей (overflow ячеек, полосы таблиц, срезы) заданы в
            // немасштабированных точках — под матрицей стопки они обязаны
            // пройти то же подобие (`Window::with_mask_scale`).
            window.with_content_mask(Some(mask), |window| {
                window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
            });
        }
        for p in 0..pages.min(self.icb.len()) {
            if self.geom(p).turn % 2 == 1 {
                continue;
            }
            let Some(mask) = masks.get(p).cloned() else {
                continue;
            };
            for el in &mut self.icb[p] {
                window.with_content_mask(Some(mask), |window| {
                    window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                });
            }
        }
        for p in 0..pages.min(self.fixed.len()) {
            if self.geom(p).turn % 2 == 1 {
                continue;
            }
            let Some(mask) = masks.get(p).cloned() else {
                continue;
            };
            for el in &mut self.fixed[p] {
                window.with_content_mask(Some(mask), |window| {
                    window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
                });
            }
        }
        // Марджин-боксы — ПОСЛЕДНИМ слоем (css-page-3 §painting: «page-margin
        // boxes» после содержимого документа), под маской всего листа.
        let sheets: Vec<Bounds<Pixels>> = (0..pages)
            .map(|p| {
                let (sx, sy) = self.sheet_origin(p);
                let g = self.geom(p);
                Bounds {
                    origin: point(bounds.origin.x + px(sx * s), bounds.origin.y + px(sy * s)),
                    size: size(px(g.size.0 * s), px(g.size.1 * s)),
                }
            })
            .collect();
        for (p, el) in &mut self.margin_els {
            if self.geoms.borrow().get(*p).is_some_and(|g| g.turn % 2 == 1) {
                continue;
            }
            let Some(sheet) = sheets.get(*p).copied() else {
                continue;
            };
            let mask = gpui::ContentMask { bounds: sheet };
            window.with_content_mask(Some(mask), |window| {
                window.with_mask_scale(bounds.origin, s, |window| el.paint(window, cx))
            });
        }
    }
}
