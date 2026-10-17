//! Lay out paginated children against the fragmentainer grid
//! (`fragment::paginate`).
//!
//! The column height `H` and the child's offset `y0` in its first column are
//! inputs of the child's layout (Blink `ConstraintSpace::FragmentainerBlockSize`
//! and `FragmentainerOffset`). With `column-fill: auto` `H` is given; when
//! balancing, `H` starts at the unfragmented balance and grows by the minimal
//! space shortage until the content with its struts fits the columns (Blink
//! `column_layout_algorithm.cc` `CalculateBalancedColumnBlockSize` +
//! `minimal_space_shortage`).

use crate::layout::fragment::paginate::solve::{Geom, first_violation};
use crate::layout::fragment::paginate::{Paginate, watch};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, AvailableSpace, Window, px, size};
use std::collections::HashMap;

const INNER: usize = 24;
const OUTER: usize = 12;

impl ColumnStack {
    pub(crate) fn paginate_children(&mut self, window: &mut Window, cx: &mut App) {
        if self.rows.is_some() || self.axis.is_vertical() {
            return;
        }
        for i in 0..self.children.len() {
            if self.children[i].paginate.is_some() && self.children[i].measure.is_some() {
                self.paginate_one(i, window, cx);
            }
        }
    }

    /// Column height and the child's offset in its first column, plus the
    /// number of columns the plan uses.
    fn frame_of(&self, i: usize) -> (f32, f32, usize) {
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
                || kid.h < 0.5
                || plan
                    .iter()
                    .filter(|f| f.kid == k)
                    .map(|f| f.from + f.h)
                    .fold(0.0f32, f32::max)
                    >= kid.h - 0.5
        });
        let used = plan.iter().map(|f| f.col + 1).max().unwrap_or(1);
        (colh, y0, if fits { used } else { usize::MAX })
    }

    /// Rebuild with `struts`, lay the main copy out, read candidate boxes.
    fn lay_out(
        &mut self,
        i: usize,
        p: &Paginate,
        struts: &HashMap<u64, f32>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Vec<Geom>> {
        let w = self.children[i].measure?;
        let n = self.children[i].frags.len() + 1;
        let (el, frags) = (p.rebuild)(struts, n)?;
        let c = &mut self.children[i];
        c.el = el;
        c.frags = frags;
        watch::reset();
        let (sz, content) = c.el.layout_as_root_with_content(
            size(AvailableSpace::Definite(px(w)), AvailableSpace::MaxContent),
            window,
            cx,
        );
        c.h = f32::from(sz.height);
        c.over = c.h.max(f32::from(content.height));
        let mut geoms = Vec::with_capacity(p.cands.len());
        for &(id, cand) in &p.cands {
            let lid = watch::seen(id)?;
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

    fn paginate_one(&mut self, i: usize, window: &mut Window, cx: &mut App) {
        let Some(p) = self.children[i].paginate.take() else {
            return;
        };
        // The flow thread is cut at the column edges: estimated break points
        // of the shape pass do not apply to the laid-out copy.
        {
            let c = &mut self.children[i];
            c.cuts.clear();
            c.forced.clear();
            c.solid.clear();
            c.monolith = false;
            c.overflow_top = false;
        }
        let balanced = self.fixed_height.is_none();
        let mut struts: HashMap<u64, f32> = HashMap::new();
        let ok = 'solve: {
            if self.lay_out(i, &p, &struts, window, cx).is_none() {
                break 'solve false;
            }
            let (mut colh, _, _) = self.frame_of(i);
            for _ in 0..OUTER {
                if balanced {
                    self.forced_h = Some(colh);
                }
                struts.clear();
                let mut shortage = f32::MAX;
                for _ in 0..INNER {
                    let Some(geoms) = self.lay_out(i, &p, &struts, window, cx) else {
                        break 'solve false;
                    };
                    let (h, y0, _) = self.frame_of(i);
                    let Some((id, delta, tall)) = first_violation(&geoms, h, y0) else {
                        break;
                    };
                    // A monolith pushed past a boundary would have fitted a
                    // column taller by what it overhangs it.
                    let need = if tall > h { tall - h } else { tall - delta };
                    if need > 0.01 {
                        shortage = shortage.min(need);
                    }
                    let s = struts.entry(id).or_insert(0.0);
                    *s = (*s + delta).max(0.0);
                }
                if !balanced {
                    break;
                }
                let (h, y0, used) = self.frame_of(i);
                if used <= self.count {
                    break;
                }
                // Content still overflows the columns: grow the columns by the
                // minimal shortage (at least the overflow spread over them).
                let end = y0 + self.children[i].h.max(self.children[i].over);
                let spread = ((end - h * self.count as f32) / self.count as f32).max(0.5);
                colh = h + if shortage < f32::MAX {
                    shortage
                } else {
                    spread
                };
            }
            true
        };
        if !ok {
            self.forced_h = None;
            struts.clear();
            let _ = self.lay_out(i, &p, &struts, window, cx);
        }
        self.children[i].paginate = Some(p);
    }
}
