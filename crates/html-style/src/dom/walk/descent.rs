//! Child traversal inputs for full cascade and targeted probes.
use super::{
    pending::{Pending, Restore},
    sibling_survey::{Survey, Visit},
};
use crate::budget::admission::{Buffer, Error};
use crate::dom::{NodeKey, shadow::Scope, tree_index::TreeIndex};
use crate::style::{css::VarMap, select::Ancestor};
use crate::values::ValueCx;
use std::rc::Rc;

pub(super) fn visits<'a>(
    survey: Rc<Survey>,
    scope: Rc<Scope>,
    vars: Rc<VarMap>,
    preserve: bool,
    depth: usize,
    vcx: ValueCx<'a>,
    parent_share: u64,
) -> Result<Buffer<Visit<'a>>, Error> {
    let mut jobs = Buffer::default();
    let admission = crate::budget::admission::Admission::new(vcx.doc.revision_budget());
    let mut pos = 0;
    for index in 0..survey.nodes.len() {
        let visit = Visit {
            survey: survey.clone(),
            index,
            pos,
            scope: scope.clone(),
            vars: vars.clone(),
            preserve,
            depth,
            vcx,
            parent_share,
        };
        pos += usize::from(survey.spots[index].index != 0);
        jobs.push(visit, &admission)?;
    }
    Ok(jobs)
}
/// Whether the children of `p` keep newlines (its `white-space`).
pub(super) fn keeps(p: &Pending<'_>) -> bool {
    p.style
        .inh
        .text_flow
        .preserve_newlines
        .unwrap_or(p.visit.preserve)
}
pub(super) fn descend<'a>(
    p: &Pending<'a>,
    tree: &TreeIndex,
    path: &mut Vec<Ancestor>,
) -> Result<(Buffer<Visit<'a>>, Restore, Option<NodeKey>), Error> {
    let admission = tree.admission();
    let restore = Restore::Truncate(path.len());
    let keep = keeps(p);
    if p.hidden && p.style.reset_context.col_role != Some(1) {
        return Ok((Buffer::default(), restore, None));
    }
    if !p.hidden {
        if let Some(shadow) = &p.shadow {
            let old = std::mem::replace(path, vec![shadow.marker.clone()]);
            return Ok((
                visits(
                    Survey::children(&shadow.root, tree),
                    shadow.scope.clone(),
                    p.vars.clone(),
                    keep,
                    p.visit.depth + 1,
                    p.visit.vcx,
                    0,
                )?,
                Restore::Replace(old),
                Some(shadow.root.key()),
            ));
        }
        if let Some(slot) = tree
            .slot_of(p.visit.handle())
            .filter(|s| !s.assigned.is_empty())
        {
            let old = std::mem::replace(path, slot.host_path.as_ref().clone());
            let survey = Rc::new(Survey {
                nodes: slot.light.clone(),
                spots: slot.light_spots.clone(),
                all: slot.light_all.clone(),
            });
            let mut jobs = Buffer::default();
            for s in &slot.assigned {
                jobs.push(
                    Visit {
                        survey: survey.clone(),
                        index: s.light_idx,
                        pos: s.elem_pos,
                        scope: slot.outer.clone(),
                        vars: p.vars.clone(),
                        preserve: keep,
                        depth: p.visit.depth + 1,
                        vcx: p.visit.vcx,
                        parent_share: 0,
                    },
                    &admission,
                )?;
            }
            let sequence = slot.host_path.last().map(|host| host.key.handle().key());
            return Ok((jobs, Restore::Replace(old), sequence));
        }
    }
    path.push(p.me.clone());
    Ok((
        visits(
            Survey::children(p.visit.handle(), tree),
            p.visit.scope.clone(),
            p.vars.clone(),
            keep,
            p.visit.depth + 1,
            p.visit.vcx,
            p.share,
        )?,
        restore,
        Some(p.visit.handle().key()),
    ))
}
