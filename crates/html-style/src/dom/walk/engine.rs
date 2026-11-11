//! Обход DOM с операциями входа/выхода; глубина не расходует стек потока.
use super::{
    enter::enter,
    leave::leave,
    pending::{Pending, Restore},
    sibling_survey::{Survey, Visit},
};
use crate::budget::admission::{Buffer, Error};
use crate::dom::outcome::TraversalKind;
use crate::dom::{Node, shadow::Scope, tree_index::TreeIndex};
use crate::style::{css::VarMap, generated::counters::Counters, select::Ancestor};
use crate::values::ValueCx;
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

enum Work<'a> {
    Visit(Visit<'a>),
    Leave(Box<Pending<'a>>, Restore),
}
fn visits<'a>(
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
fn descend<'a, const QUERY: bool>(
    p: &Pending<'a>,
    tree: &TreeIndex,
    path: &mut Vec<Ancestor>,
) -> Result<(Buffer<Visit<'a>>, Restore), Error> {
    let admission = tree.admission();
    let restore = Restore::Truncate(path.len());
    let keep = p
        .style
        .inh
        .text_flow
        .preserve_newlines
        .unwrap_or(p.visit.preserve);
    if !QUERY && p.hidden && p.style.reset_context.col_role != Some(1) {
        return Ok((Buffer::default(), restore));
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
            return Ok((jobs, Restore::Replace(old)));
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
    ))
}
#[allow(clippy::too_many_arguments)]
pub fn walk_children<const QUERY: bool>(
    handle: &Handle,
    tree: &TreeIndex,
    scope: &Rc<Scope>,
    vars: &VarMap,
    counter: &mut u64,
    counters: &mut Counters,
    path: &mut Vec<Ancestor>,
    preserve: bool,
    out: &mut Vec<Node>,
    vcx: ValueCx<'_>,
    mut invalidation: Option<&mut crate::dom::invalidation::Invalidation>,
) -> Result<(), crate::budget::admission::Error> {
    let admission = tree.admission();
    let mut jobs = visits(
        Survey::children(handle, tree),
        scope.clone(),
        Rc::new(vars.clone()),
        preserve,
        1,
        vcx,
        0,
    )?;
    let sharing = super::element_sharing::ElementSharing::default();
    let mut work = Buffer::default();
    while let Some(job) = jobs.pop() {
        work.push(Work::Visit(job), &admission)?;
    }
    drop(jobs);
    let mut output = Buffer::default();
    // Только запрос посещает display:none ради computed values. Его счётчики
    // изолированы от видимых соседей; обычный проход не выделяет этот стек.
    let mut hidden_counters = Vec::new();
    output.push(super::kids::Kids::from_vec(std::mem::take(out)), &admission)?;
    while let Some(next) = work.pop() {
        admission.work(1)?;
        match next {
            Work::Leave(p, restore) => {
                let hidden = QUERY && p.hidden;
                restore.apply(path);
                let children = output.pop().expect("children output").into_vec();
                leave::<QUERY>(
                    p,
                    path,
                    counter,
                    counters,
                    children,
                    output.last_mut().expect("parent output"),
                    invalidation.as_deref_mut(),
                )?;
                if hidden {
                    *counters = hidden_counters.pop().expect("hidden counter scope");
                }
            }
            Work::Visit(v) => {
                let limit = crate::values::MAX_DEPTH;
                if v.depth > limit
                    && matches!(
                        v.handle().data,
                        NodeData::Text { .. } | NodeData::Element { .. }
                    )
                {
                    tree.doc
                        .traversal_limit_reached(TraversalKind::BoxBuild, limit, v.depth);
                    continue;
                }
                match &v.handle().data {
                    NodeData::Text { contents } => super::text_nodes::walk_text(
                        crate::dom::Text::from_source(
                            contents.borrow().to_string(),
                            v.vcx.doc.traversal.record_text(v.handle()),
                        ),
                        v.preserve,
                        &v.survey.nodes,
                        v.index,
                        output.last_mut().expect("output"),
                    ),
                    NodeData::Element { .. } => {
                        let saved = QUERY.then(|| counters.clone());
                        if let Some(mut p) = enter::<QUERY>(
                            v,
                            tree,
                            &sharing,
                            counter,
                            counters,
                            path,
                            output.last_mut().expect("output"),
                        ) {
                            if QUERY && p.hidden {
                                hidden_counters.push(saved.expect("query counters"));
                            }
                            let (mut children, restore) = descend::<QUERY>(&p, tree, path)?;
                            output.push(std::mem::take(&mut p.children), &admission)?;
                            work.push(Work::Leave(p, restore), &admission)?;
                            while let Some(job) = children.pop() {
                                work.push(Work::Visit(job), &admission)?;
                            }
                        }
                    }
                    _ => {
                        let mut jobs = visits(
                            Survey::children(v.handle(), tree),
                            v.scope.clone(),
                            v.vars.clone(),
                            v.preserve,
                            v.depth,
                            v.vcx,
                            0,
                        )?;
                        while let Some(job) = jobs.pop() {
                            work.push(Work::Visit(job), &admission)?;
                        }
                    }
                }
            }
        }
    }
    *out = output.pop().expect("root output").into_vec();
    Ok(())
}
