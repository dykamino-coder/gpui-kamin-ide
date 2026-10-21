//! Рамка фрагментаинера размечаемого ребёнка стопки (высота колонки, смещение в первой
//! колонке, число занятых колонок) и его измерительная раскладка: пересборка с распорками,
//! раскладка основной копии и чтение коробок-кандидатов разрыва.

use crate::layout::fragment::paginate::solve::Geom;
use crate::layout::fragment::paginate::{Cand, Paginate, Struts};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, AvailableSpace, Window, point, px, size};

impl ColumnStack {
    /// Column height and the child's offset in its first column, plus the
    /// number of columns the plan uses.
    pub(super) fn frame_of(&self, i: usize) -> (f32, f32, usize) {
        let kids = self.kid_geoms();
        let (h, lines, plan, _) = self.balance(&kids);
        let colh = self
            .fixed_height
            .or(self.forced_h)
            .or(lines.first().map(|l| l.1))
            .unwrap_or(h);
        let y0 = plan
            .iter()
            .find(|f| f.kid == i && f.copy == 0)
            .map_or(0.0, |f| f.y);
        // Every child's content must be placed within the columns.
        let fits = kids.iter().enumerate().all(|(k, kid)| {
            kid.span
                || kid.h.max(kid.over) < 0.5
                || plan
                    .iter()
                    .filter(|f| f.kid == k)
                    .map(|f| f.from + f.h)
                    .fold(0.0f32, f32::max)
                    >= kid.h.max(kid.over) - 0.5
        });
        let used = plan.iter().map(|f| f.col + 1).max().unwrap_or(1);
        (colh, y0, if fits { used } else { usize::MAX })
    }

    /// Rebuild with `struts`, lay the main copy out, read candidate boxes.
    pub(super) fn lay_out(
        &mut self,
        i: usize,
        p: &Paginate,
        n: usize,
        struts: &Struts,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Vec<Geom>> {
        let w = self.children[i].measure?;
        let (el, frags) = (p.rebuild)(struts, n)?;
        let c = &mut self.children[i];
        c.el = el;
        c.frags = frags;
        p.watched.reset();
        let (sz, content) = c.el.layout_as_root_with_content(
            size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent),
            window,
            cx,
        );
        c.h = f32::from(sz.height);
        c.over = c.h.max(f32::from(content.height));
        let measured = p
            .watched
            .flow
            .at(point(px(0.0), px(0.0)), sz, self.axis, window);
        let mut geoms = Vec::with_capacity(p.cands.len());
        for &(id, cand) in &p.cands {
            if matches!(cand, Cand::Float { .. }) {
                let g = measured.floats.iter().find(|g| g.id == id)?;
                // Float margins are unbreakable and never truncated (Blink
                // floats_utils.cc, CreateConstraintSpace and initial balancing).
                geoms.push(Geom {
                    id,
                    cand,
                    top: g.top - g.margins[0],
                    bottom: g.first_break.unwrap_or(g.bottom + g.margins[1]),
                });
                continue;
            }
            let lid = p.watched.seen(id)?;
            let o = window.layout_origin_unrounded(lid);
            let s = window.layout_size_unrounded(lid);
            geoms.push(Geom {
                id,
                cand,
                top: f32::from(o.y),
                bottom: f32::from(o.y + s.height),
            });
        }
        Some(geoms)
    }
}
