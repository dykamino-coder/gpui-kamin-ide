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
use crate::layout::fragment::paginate::{Cand, Paginate, Strut, Struts, watch};
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{App, AvailableSpace, Window, px, size};

mod saved;
use saved::Saved;

const INNER: usize = 24;
const OUTER: usize = 12;

impl ColumnStack {
    pub(crate) fn paginate_children(&mut self, window: &mut Window, cx: &mut App) {
        if self.rows.is_some_and(|r| !r.cap) || self.axis.is_vertical() {
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
    fn lay_out(
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
        // The shape-estimate record, restored when pagination fails.
        let n = self.children[i].frags.len() + 1;
        let saved = Saved::take(&mut self.children[i]);
        // The flow thread is cut at the column edges: estimated break points
        // of the shape pass do not apply to the laid-out copy.
        {
            let c = &mut self.children[i];
            c.cuts.clear();
            c.forced.clear();
            c.solid.clear();
            c.monolith = p.avoid_root;
            c.par.avoid_only = p.avoid_root;
            c.overflow_top = false;
        }
        let balanced = self.fixed_height.is_none();
        let mut struts: Struts = Struts::new();
        let ok = 'solve: {
            let Some(geoms) = self.lay_out(i, &p, n, &struts, window, cx) else {
                break 'solve false;
            };
            // First guess: the ordinary balance told where the unfragmented
            // copy has forced breaks and unbreakable ranges (legacy content
            // runs, Blink `ColumnBalancer`), so it starts from the tallest run.
            {
                let c = &mut self.children[i];
                for g in &geoms {
                    match g.cand {
                        Cand::Forced { mt } => c.forced.push(g.top - mt),
                        Cand::Mono { .. } => c.solid.push((g.top, g.bottom)),
                        Cand::Item => {}
                    }
                }
                c.forced.sort_by(f32::total_cmp);
            }
            // Runs end at the end of the parallel flow, not of the box.
            let own = self.children[i].h;
            self.children[i].h = own.max(self.children[i].over);
            // Main sizes of column-flex items before fragmentation.
            let frozen: Struts = geoms
                .iter()
                .filter(|g| g.cand == Cand::Item)
                .map(|g| {
                    (
                        g.id,
                        Strut {
                            by: 0.0,
                            size: g.bottom - g.top,
                        },
                    )
                })
                .collect();
            let (mut colh, _, _) = self.frame_of(i);
            {
                let c = &mut self.children[i];
                c.h = own;
                c.forced.clear();
                c.solid.clear();
            }
            for _ in 0..OUTER {
                if balanced {
                    self.forced_h = Some(colh);
                }
                struts = frozen.clone();
                let mut shortage = f32::MAX;
                for _ in 0..INNER {
                    let Some(geoms) = self.lay_out(i, &p, n, &struts, window, cx) else {
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
                    let size = geoms
                        .iter()
                        .find(|g| g.id == id)
                        .map_or(0.0, |g| g.bottom - g.top);
                    let s = struts.entry(id).or_insert(Strut { by: 0.0, size });
                    s.by = (s.by + delta).max(0.0);
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
                // Never taller than the container allows (Blink
                // `ConstrainColumnBlockSize`); the rest overflows. `rule_to` is
                // the content block size of a definite-height container.
                let cap = self
                    .rows
                    .and_then(|r| r.h)
                    .or(self.rule_to)
                    .unwrap_or(f32::MAX);
                if h >= cap - 0.01 {
                    break;
                }
                let step = if shortage < f32::MAX {
                    shortage
                } else {
                    spread
                };
                colh = (h + step).min(cap);
            }
            true
        };
        if !ok {
            self.forced_h = None;
            saved.restore(&mut self.children[i]);
        }
        self.children[i].paginate = Some(p);
    }
}
