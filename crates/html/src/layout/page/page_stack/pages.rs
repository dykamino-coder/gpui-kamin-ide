//! Предраскладка фрагментов по листам и слоёв ICB/fixed.

use super::{PageGeom, PageStack};
use crate::layout::fragment::types::{Frag, Kid};
use gpui::{App, Bounds, Pixels, Window, point, px, size};

impl PageStack {
    #[allow(clippy::too_many_arguments)]
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
        for f in plan {
            let (sx, sy) = self.sheet_origin(f.col);
            let pg = geoms.get(f.col).copied().unwrap_or(g);
            let (ax, ay) = pg.area_origin();
            let aw = pg.area.0.max(1.0);
            let full_h = kids[f.kid].h;
            let kid = &mut self.kids[f.kid];
            let inner_top = kid.inner_top;
            let el = if f.copy == 0 {
                &mut kid.el
            } else {
                match kid.frags.get_mut(f.copy - 1) {
                    Some(e) => e,
                    None => continue,
                }
            };
            el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(aw)),
                    gpui::AvailableSpace::Definite(px(full_h)),
                ),
                window,
                cx,
            );
            el.prepaint_at(
                point(
                    bounds.origin.x + px(sx + ax),
                    bounds.origin.y + px(sy + ay + f.y - f.from - inner_top),
                ),
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
                el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::Definite(px(ah)),
                    ),
                    window,
                    cx,
                );
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
                el.layout_as_root(
                    size(
                        gpui::AvailableSpace::Definite(px(aw)),
                        gpui::AvailableSpace::Definite(px(ah)),
                    ),
                    window,
                    cx,
                );
                el.prepaint_at(
                    point(bounds.origin.x + px(sx + ax), bounds.origin.y + px(sy + ay)),
                    window,
                    cx,
                );
            }
        }
    }
}
