//! Предраскладка фрагментов по листам и слоёв ICB/fixed.

use super::{PageGeom, PageStack};
use crate::layout::fragment::types::{Frag, Kid};
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl PageStack {
    #[allow(clippy::too_many_arguments)]
    // Page indices address both the plan and parallel per-page layout state.
    #[allow(clippy::needless_range_loop)]
    pub(super) fn prepaint_pages(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
        g: PageGeom,
        aw: f32,
        ah: f32,
        kids: Vec<Kid>,
        geoms: &[PageGeom],
        plan: &Vec<Frag>,
        pages: usize,
    ) {
        // ICB and `fixed` layers are laid out BEFORE the kid copies prepaint:
        // an abspos split by `source_paint::split` keeps its layout half in a
        // layer and prepaints from its source-order half inside the kid
        // (CSS 2.1 Appendix E), so the layer's layout must exist by then
        // (`transform-023-print` aborted with "must call request_layout
        // before prepaint"). Every copy is laid out: copy `c` of a kid pairs
        // with layer copy `c`, wherever the kid lands.
        let space = size(
            gpui::AvailableSpace::Definite(px(aw)),
            gpui::AvailableSpace::Definite(px(ah)),
        );
        for el in self.icb.iter_mut().chain(self.fixed.iter_mut()).flatten() {
            el.layout_as_root(space, window, cx);
        }
        for f in plan {
            let (sx, sy) = self.sheet_origin(f.col);
            let pg = geoms.get(f.col).copied().unwrap_or(g);
            let (ax, ay) = pg.area_origin();
            let aw = pg.area.0.max(1.0);
            let full_h = kids[f.kid].h;
            let inner_top = self.kids[f.kid].inner_top;
            // The copy is laid out WHOLE and shifted onto its slice; physical
            // box by the root's block axis (`page_stack/axis.rs`).
            let (x, y, w, h) = if self.axis.is_vertical() {
                self.area_slice(&pg, f.y - f.from, full_h)
            } else {
                (ax, ay + f.y - f.from - inner_top, aw, full_h)
            };
            let kid = &mut self.kids[f.kid];
            if let Some(r) = kid.repeat.as_mut() {
                r.place_bands(f, |el, at| {
                    el.layout_as_root(
                        size(
                            gpui::AvailableSpace::Definite(px(w)),
                            gpui::AvailableSpace::Definite(px(h)),
                        ),
                        window,
                        cx,
                    );
                    el.prepaint_at(
                        point(
                            bounds.origin.x + px(sx + ax),
                            bounds.origin.y + px(sy + ay + f.y + at - inner_top),
                        ),
                        window,
                        cx,
                    );
                });
            }
            let Some(el) =
                crate::layout::fragment::types::frag_copy(&mut kid.el, &mut kid.frags, f.copy)
            else {
                continue;
            };
            el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(w)),
                    gpui::AvailableSpace::Definite(px(h)),
                ),
                window,
                cx,
            );
            el.prepaint_at(
                point(bounds.origin.x + px(sx + x), bounds.origin.y + px(sy + y)),
                window,
                cx,
            );
        }
        // Слой ICB — копия `p` на листе `p`, в его page area, поднятая на `p`
        // высот area: непрерывный поток абсолютов, разрезанный страницами.
        for p in 0..pages.min(self.icb.len()) {
            let (sx, sy) = self.sheet_origin(p);
            let (ax, ay) = geoms[p].area_origin();
            let lift = p as f32 * ah;
            for el in &mut self.icb[p] {
                el.prepaint_at(
                    point(
                        bounds.origin.x + px(sx + ax),
                        bounds.origin.y + px(sy + ay - lift),
                    ),
                    window,
                    cx,
                );
            }
        }
        // Слой `fixed` — копия `p` на листе `p` без сдвига: page area каждого
        // листа — его содержащий блок.
        for p in 0..pages.min(self.fixed.len()) {
            let (sx, sy) = self.sheet_origin(p);
            // Размер содержащего блока — page area ПЕРВОГО листа: Blink
            // раскладывает `fixed` один раз от начального содержащего блока и
            // повторяет на каждом листе (`fixedpos-010-print`: `right: -100px`
            // при листе 400 — за краем, на листах 500 — в правом нижнем углу).
            let (ax, ay) = geoms[p].area_origin();
            for el in &mut self.fixed[p] {
                el.prepaint_at(
                    point(bounds.origin.x + px(sx + ax), bounds.origin.y + px(sy + ay)),
                    window,
                    cx,
                );
            }
        }
    }
}
