//! Независимый OOF-план страниц: actual Space, immutable исходный CB, последний fixed.

use super::{PageGeom, PageStack, spaces::Pages};
use crate::layout::fragment::plan::oof::stream::Streams;
use crate::layout::fragment::plan::{Completion, Plan};
use crate::layout::fragment::token::Error;
use crate::layout::positioned::containing_block::metrics::fragmentation::Source;
use gpui::{App, Window};
use std::cell::RefCell;
mod source;

#[derive(Default)]
pub(super) struct State {
    pub(super) plan: RefCell<Plan>,
    sources: RefCell<Vec<(usize, Source)>>,
    fixed: RefCell<Vec<(usize, usize, Source)>>,
}

#[cfg(test)]
mod tests;

pub(super) struct Normal<'a> {
    pub(super) plan: &'a Plan,
    pub(super) geoms: &'a [PageGeom],
    pub(super) names: &'a [String],
    pub(super) count: usize,
}

struct Context<'a> {
    normal: &'a Plan,
    pages: Pages<'a>,
    count: usize,
}

impl PageStack {
    pub(super) fn plan_oof(&self, normal: Normal<'_>, window: &mut Window, cx: &mut App) {
        let mut plan = Plan::default();
        self.oof.fixed.borrow_mut().clear();
        self.oof.sources.borrow_mut().clear();
        let mut interned = normal.names.to_vec();
        interned.sort();
        interned.dedup();
        let context = Context {
            normal: normal.plan,
            count: normal.count,
            pages: Pages {
                names: normal.names,
                interned: &interned,
                geoms: normal.geoms,
                geom_for: &self.geom_for,
                principal: self.principal,
            },
        };
        if let Err(error) = self.resume_oof(&context, window, cx, &mut plan) {
            plan.failure = Some(error);
            plan.incomplete();
        }
        if normal.plan.completion != Completion::Complete {
            plan.incomplete();
        }
        *self.oof.plan.borrow_mut() = plan;
    }

    fn resume_oof(
        &self,
        context: &Context<'_>,
        window: &mut Window,
        cx: &mut App,
        result: &mut Plan,
    ) -> Result<(), Error> {
        let (sources, inputs) = self.oof_inputs(context.normal, &context.pages, window, cx)?;
        let mut streams = Streams::new(inputs)?;
        let mut index = 0;
        while index < context.count || streams.needs_space() {
            self.refresh_fixed(&sources, context, index, &mut streams, window, cx)?;
            let batch = streams.resume(
                index,
                context.pages.at(index),
                index + 1 >= context.count && context.normal.completion == Completion::Complete,
            )?;
            result
                .fragments
                .extend(batch.fragments.into_iter().map(|(id, mut f)| {
                    f.kid = id as usize;
                    f
                }));
            index = index.checked_add(1).ok_or(Error::ResourceExhausted)?;
        }
        *self.oof.sources.borrow_mut() = sources;
        Ok(())
    }
}
