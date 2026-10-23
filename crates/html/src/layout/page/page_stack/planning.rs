//! Согласование имён листов, их размеров и роста фрагментируемого flex.
use super::{PageGeom, PageStack};
use crate::layout::fragment::plan::{self, Space};
use crate::layout::fragment::types::{Frag, Kid};
use gpui::{App, Window};
mod settlement;
use settlement::{Incomplete, State, settle};

pub(super) type PlannedPages = (Vec<Kid>, Vec<PageGeom>, Vec<String>, usize, Vec<Frag>);
impl PageStack {
    pub(super) fn fragment_plan(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        g: PageGeom,
        area: (f32, f32),
    ) -> Result<PlannedPages, Incomplete> {
        let first = self
            .kids
            .first()
            .map(|k| k.page.clone())
            .unwrap_or_default();
        let (mut kids, limit) = self.page_kids(window, cx, (area.0.max(1.0), area.1.max(1.0)));
        let kid_names: Vec<_> = self.kids.iter().map(|k| k.page.clone()).collect();
        let mut geoms = vec![g];
        let mut names = vec![first.clone()];
        let result = settle(|| {
            let tail = names.last().cloned().unwrap_or_default();
            self.measured.clear();
            for (index, kid) in self.kids.iter().enumerate() {
                if let Some(r) = &kid.regrow {
                    self.measured.insert(
                        (index, 0),
                        super::continuation::CopyGeometry {
                            width: area.0.max(1.0),
                            kid: kids[index].clone(),
                            laid: r.laid.clone(),
                            continued: false,
                        },
                    );
                }
            }
            let geom_for = self.geom_for.clone();
            let axis = self.axis;
            let heights = |col| {
                let pg = geoms
                    .get(col)
                    .copied()
                    .unwrap_or_else(|| geom_for(col, &tail));
                if axis.is_vertical() {
                    pg.area.0.max(1.0)
                } else {
                    pg.area.1.max(1.0)
                }
            };
            let (pages, _, plan) = plan::fill_measured(
                &kids,
                &heights,
                Space::pages(limit),
                &mut |kid, copy, col, k, from| {
                    let pg = geoms
                        .get(col)
                        .copied()
                        .unwrap_or_else(|| geom_for(col, &tail));
                    let area = if axis.is_vertical() {
                        (pg.area.1, pg.area.0)
                    } else {
                        pg.area
                    };
                    self.remeasure_copy(kid, copy, area, k, from, window, cx)
                },
            );
            let extents: Vec<_> = (0..pages).map(heights).collect();
            let state = State::new(&names, &geoms, &plan, &self.measured);
            if self.regrow_flex(&kids, &plan, &extents) {
                (kids, _) = self.page_kids(window, cx, (area.0.max(1.0), area.1.max(1.0)));
                return (false, state, (pages, plan));
            }
            let mut new_names: Vec<Option<String>> = vec![None; pages];
            for f in &plan {
                if let Some(slot) = new_names.get_mut(f.col)
                    && slot.is_none()
                {
                    *slot = Some(kid_names[f.kid].clone());
                }
            }
            let mut last = first.clone();
            let new_names: Vec<_> = new_names
                .into_iter()
                .map(|n| {
                    if let Some(n) = n {
                        last = n;
                    }
                    last.clone()
                })
                .collect();
            let stable = new_names == names;
            names = new_names;
            geoms = names
                .iter()
                .enumerate()
                .map(|(i, n)| (self.geom_for)(i, n))
                .collect();
            (stable, state, (pages, plan))
        })?;
        for (pk, k) in self.kids.iter().zip(&kids) {
            pk.end.set(k.h);
        }
        Ok((kids, geoms, names, result.0, result.1))
    }
}

#[cfg(test)]
mod tests;
