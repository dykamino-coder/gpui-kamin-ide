//! Согласование имён листов, их размеров и роста фрагментируемого flex.
use super::{PageGeom, PageStack};
use crate::layout::fragment::plan::{self, Space};
use crate::layout::fragment::types::{Frag, Kid};
use gpui::{App, Window};

pub(super) type PlannedPages = (Vec<Kid>, Vec<PageGeom>, Vec<String>, usize, Vec<Frag>);
impl PageStack {
    pub(super) fn fragment_plan(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        g: PageGeom,
        area: (f32, f32),
    ) -> PlannedPages {
        let first = self
            .kids
            .first()
            .map(|k| k.page.clone())
            .unwrap_or_default();
        let (mut kids, limit) = self.page_kids(window, cx, (area.0.max(1.0), area.1.max(1.0)));
        let kid_names: Vec<_> = self.kids.iter().map(|k| k.page.clone()).collect();
        let mut geoms = vec![g];
        let mut names = vec![first.clone()];
        let mut result = (0, Vec::new());
        for pass in 0..=6 {
            let tail = names.last().cloned().unwrap_or_default();
            let heights = |col| {
                let pg = geoms
                    .get(col)
                    .copied()
                    .unwrap_or_else(|| (self.geom_for)(col, &tail));
                self.area_logical(&pg).1.max(1.0)
            };
            let (pages, _, plan) = plan::fill_at(&kids, &heights, Space::pages(limit));
            let extents: Vec<_> = (0..pages).map(heights).collect();
            if pass < 6 && self.regrow_flex(&kids, &plan, &extents) {
                (kids, _) = self.page_kids(window, cx, (area.0.max(1.0), area.1.max(1.0)));
                continue;
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
            result = (pages, plan);
            if stable {
                break;
            }
        }
        for (pk, k) in self.kids.iter().zip(&kids) {
            pk.end.set(k.h);
        }
        (kids, geoms, names, result.0, result.1)
    }
}
