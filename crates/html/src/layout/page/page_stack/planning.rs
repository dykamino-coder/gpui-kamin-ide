//! Согласование имён листов, их размеров и роста фрагментируемого flex.
use super::{PageGeom, PageStack};
use crate::layout::fragment::plan::{self, Plan, Space};
use crate::layout::fragment::token::Error;
use crate::layout::fragment::types::Kid;
use gpui::{App, Window};
mod settlement;
use settlement::{Incomplete, State, settle};

pub(super) type PlannedPages = (Vec<Kid>, Vec<PageGeom>, Vec<String>, usize, Plan);
#[derive(Debug)]
pub(super) enum Failure {
    Settlement,
    Continuation(Error),
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Settlement => f.write_str("page geometry did not converge"),
            Self::Continuation(error) => error.fmt(f),
        }
    }
}
impl PageStack {
    pub(super) fn fragment_plan(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        g: PageGeom,
        area: (f32, f32),
    ) -> Result<PlannedPages, Failure> {
        let first = self
            .kids
            .first()
            .map(|k| k.page.clone())
            .unwrap_or_default();
        let (mut kids, mut inline, limit) =
            self.page_kids(window, cx, (area.0.max(1.0), area.1.max(1.0)));
        let kid_names: Vec<_> = self.kids.iter().map(|k| k.page.clone()).collect();
        let mut interned = kid_names.clone();
        interned.sort();
        interned.dedup();
        let mut geoms = vec![g];
        let mut names = vec![first.clone()];
        let result = settle(|| {
            let geom_for = self.geom_for.clone();
            let pages = super::spaces::Pages {
                names: &names,
                interned: &interned,
                geoms: &geoms,
                geom_for: &geom_for,
                principal: self.principal,
            };
            self.measured.clear();
            for (index, kid) in self.kids.iter().enumerate() {
                if let Some(r) = &kid.regrow {
                    self.measured.insert(
                        (index, 0),
                        super::continuation::CopyGeometry {
                            inputs: super::continuation::Inputs {
                                constraints: crate::layout::cx::FragmentConstraints {
                                    area: (area.0.max(1.0), area.1.max(1.0)),
                                    parent: r.opts.cx.constraints.clone(),
                                },
                                source: r.space,
                                page: pages.at(0).page,
                            },
                            from: 0.0,
                            offset: 0.0,
                            kid: kids[index].clone(),
                            inline: inline[index].clone(),
                            laid: r.laid.clone(),
                            parallel: None,
                            continued: false,
                        },
                    );
                }
            }
            let axis = self.axis;
            let heights = |col| pages.at(col).block_size.unwrap();
            let leaves: Vec<_> = self
                .kids
                .iter()
                .map(|p| p.leaf.as_ref().map(|l| l.geometry.clone()))
                .collect();
            let mut continuation_error = None;
            let planned = plan::fill_resumed(
                &kids,
                &|col| pages.at(col),
                Space::pages(limit),
                &|index, space| leaves[index].as_ref().map(|at| at(space)),
                &mut |request| {
                    let plan::MeasureRequest {
                        kid,
                        copy,
                        col,
                        from,
                        block_size,
                        page,
                    } = request;
                    let pg = geoms.get(col).copied().unwrap_or_else(|| {
                        let name = names.last().map_or("", String::as_str);
                        geom_for(col, name)
                    });
                    let area = if axis.is_vertical() {
                        (pg.area.1, block_size)
                    } else {
                        (pg.area.0, block_size)
                    };
                    let geometry =
                        match self.remeasure_copy(kid, copy, area, from, page, window, cx) {
                            Ok(geometry) => geometry,
                            Err(error) => {
                                continuation_error = Some(error);
                                None
                            }
                        };
                    plan::Measurement {
                        copy: geometry,
                        inline: self
                            .measured
                            .get(&(kid, copy))
                            .map_or_else(|| inline[kid].clone(), |m| m.inline.clone()),
                    }
                },
            );
            if let Some(error) = continuation_error {
                return (
                    true,
                    State::new(&names, &geoms, &[], &self.measured),
                    Err(error),
                );
            }
            let (pages, _, plan) = match planned {
                Ok(plan) => plan,
                Err(error) => {
                    return (
                        true,
                        State::new(&names, &geoms, &[], &self.measured),
                        Err(error),
                    );
                }
            };
            let extents: Vec<_> = (0..pages).map(heights).collect();
            let state = State::new(&names, &geoms, &plan, &self.measured);
            if self.regrow_flex(&kids, &plan, &extents) {
                (kids, inline, _) = self.page_kids(window, cx, (area.0.max(1.0), area.1.max(1.0)));
                return (false, state, Ok((pages, plan)));
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
            (stable, state, Ok((pages, plan)))
        })
        .map_err(|Incomplete| Failure::Settlement)?
        .map_err(Failure::Continuation)?;
        for (index, kid) in self.kids.iter_mut().enumerate() {
            if let Some(leaf) = &kid.leaf {
                let count = result
                    .1
                    .iter()
                    .filter(|f| f.kid == index)
                    .count()
                    .checked_sub(1)
                    .ok_or(Failure::Continuation(Error::InvalidToken))?;
                kid.frags.clear();
                kid.frags
                    .try_reserve(count)
                    .map_err(|_| Failure::Continuation(Error::ResourceExhausted))?;
                let pages = super::spaces::Pages {
                    names: &names,
                    interned: &interned,
                    geoms: &geoms,
                    geom_for: &self.geom_for,
                    principal: self.principal,
                };
                for f in result.1.iter().filter(|f| f.kid == index) {
                    if f.copy == 0 {
                        kid.el = (leaf.build)(pages.at(f.col), *f);
                    } else {
                        kid.frags.push((leaf.build)(pages.at(f.col), *f));
                    }
                }
            }
        }
        for (pk, k) in self.kids.iter().zip(&kids) {
            pk.end.set(k.h);
        }
        Ok((kids, geoms, names, result.0, result.1))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod completion_tests;
