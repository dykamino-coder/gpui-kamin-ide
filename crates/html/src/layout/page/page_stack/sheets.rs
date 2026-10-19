//! Краска самих листов: фон, рамка и марджин-боксы (css-page-3 §painting).

use super::PageStack;
use crate::layout::fragment::types::Frag;
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl PageStack {
    /// Маска `m` (page area листа `i` или срез фрагмента), расширенная по оси
    /// строки до краёв листа. Переполнение ПОТОКА за page area в поля листа
    /// видно (css-page-3 §page-model: page area — фрагментаинер, режет лишь
    /// лист): левый свес глифа `A` у края page area срезался, а эталон без
    /// полей его рисует (`margin-boxes/content-004..013`). Слой `fixed` Blink
    /// режет по page area (`position-fixed-overflow-print`) — ему эта маска не
    /// нужна. По оси блока режет срез фрагмента.
    fn sheet_wide(
        &self,
        bounds: Bounds<Pixels>,
        i: usize,
        m: gpui::ContentMask<Pixels>,
    ) -> gpui::ContentMask<Pixels> {
        let s = self.grid.get().1;
        let (sx, sy) = self.sheet_origin(i);
        let (w, h) = self.geom(i).size;
        let mut b = m.bounds;
        // The inline axis of a vertical root is y (`page_stack/axis.rs`).
        if self.axis.is_vertical() {
            b.origin.y = bounds.origin.y + px(sy * s);
            b.size.height = px(h * s);
        } else {
            b.origin.x = bounds.origin.x + px(sx * s);
            b.size.width = px(w * s);
        }
        gpui::ContentMask { bounds: b }
    }

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
        // Sheet backgrounds, the canvas and page borders go to the BOTTOM
        // layer: negative `z-index` content (`Underlay`, CSS 2.1 §9.9 step 3)
        // also paints there and must land above them (css-page-3 §painting:
        // page background, canvas, borders, then the root stacking context;
        // `page-background-001-print-ref`: `<img style="z-index:-1">`).
        window.paint_bottom_layer(|window| {
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
                if !self.canvas_els.is_empty() {
                    let (x, y, w, h) = super::canvas::canvas_rect(&g);
                    self.paint_canvas(i, rect(sx + x, sy + y, w, h), bounds, s, window, cx);
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
        });
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
            // The fragment that ends its box shows the box's block-end
            // overflow up to the page area's end (overflow is visible,
            // css-overflow-3 §2; `block-001-wm-vrl`: a negative block-end
            // margin shrinks the margin box under the border box).
            let ext = self.mask_extent(&g, &f);
            let (x, y, w, h) = if self.axis.is_vertical() {
                self.area_slice(&g, f.y, ext)
            } else {
                (ax, ay + f.y, g.area.0, ext)
            };
            let mask = gpui::ContentMask {
                bounds: Bounds {
                    origin: point(
                        bounds.origin.x + px((sx + x) * s),
                        bounds.origin.y + px((sy + y) * s),
                    ),
                    size: size(px(w * s), px(h * s)),
                }
                .intersect(&page.bounds),
            };
            // По оси строки — весь лист (`PageStack::sheet_wide`).
            let mask = self.sheet_wide(bounds, f.col, mask);
            // Clip and painted box edges share the device-pixel grid.
            let mask =
                crate::layout::fragment::fragment_mask::snap(mask.bounds, window.scale_factor());
            let kid = &mut self.kids[f.kid];
            window.with_mask_scale(bounds.origin, s, |window| {
                kid.paint_sections(
                    &f,
                    |top, h| {
                        rect(sx + ax, sy + ay + f.y + top, g.area.0, h).intersect(&rect(
                            sx + ax,
                            sy + ay,
                            g.area.0,
                            g.area.1,
                        ))
                    },
                    window,
                    cx,
                );
            });
            let Some(el) =
                crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)
            else {
                continue;
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
