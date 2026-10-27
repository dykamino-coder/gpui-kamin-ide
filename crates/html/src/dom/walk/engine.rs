//! Обход DOM с операциями входа/выхода; глубина не расходует стек потока.
use super::{
    enter::enter,
    leave::leave,
    pending::{Pending, Restore},
    sibling_survey::{Survey, Visit},
};
use crate::document::state::ValueCx;
use crate::dom::outcome::TraversalKind;
use crate::dom::{Node, shadow::Scope, tree_index::TreeIndex};
use crate::style::{css::VarMap, generated::counters::Counters, select::Ancestor};
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
) -> Vec<Visit<'a>> {
    let mut pos = 0;
    (0..survey.nodes.len())
        .map(|index| {
            let visit = Visit {
                survey: survey.clone(),
                index,
                pos,
                scope: scope.clone(),
                vars: vars.clone(),
                preserve,
                depth,
                vcx,
            };
            pos += usize::from(survey.spots[index].index != 0);
            visit
        })
        .collect()
}
fn descend<'a>(
    p: &Pending<'a>,
    tree: &TreeIndex,
    path: &mut Vec<Ancestor>,
) -> (Vec<Visit<'a>>, Restore) {
    let restore = Restore::Truncate(path.len());
    let keep = p.style.preserve_newlines.unwrap_or(p.visit.preserve);
    if p.hidden && p.style.col_role != Some(1) {
        return (vec![], restore);
    }
    if !p.hidden {
        if let Some(shadow) = &p.shadow {
            let old = std::mem::replace(path, vec![shadow.marker.clone()]);
            return (
                visits(
                    Survey::children(&shadow.root, tree),
                    shadow.scope.clone(),
                    p.vars.clone(),
                    keep,
                    p.visit.depth + 1,
                    p.visit.vcx,
                ),
                Restore::Replace(old),
            );
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
            let jobs = slot
                .assigned
                .iter()
                .map(|s| Visit {
                    survey: survey.clone(),
                    index: s.light_idx,
                    pos: s.elem_pos,
                    scope: slot.outer.clone(),
                    vars: p.vars.clone(),
                    preserve: keep,
                    depth: p.visit.depth + 1,
                    vcx: p.visit.vcx,
                })
                .collect();
            return (jobs, Restore::Replace(old));
        }
    }
    path.push(p.me.clone());
    (
        visits(
            Survey::children(p.visit.handle(), tree),
            p.visit.scope.clone(),
            p.vars.clone(),
            keep,
            p.visit.depth + 1,
            p.visit.vcx,
        ),
        restore,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn walk_children(
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
) {
    let jobs = visits(
        Survey::children(handle, tree),
        scope.clone(),
        Rc::new(vars.clone()),
        preserve,
        1,
        vcx,
    );
    let mut work: Vec<_> = jobs.into_iter().rev().map(Work::Visit).collect();
    let mut output = vec![std::mem::take(out)];
    while let Some(next) = work.pop() {
        match next {
            Work::Leave(p, restore) => {
                restore.apply(path);
                let children = output.pop().expect("children output");
                leave(
                    p,
                    path,
                    counter,
                    counters,
                    children,
                    output.last_mut().expect("parent output"),
                );
            }
            Work::Visit(v) => {
                let limit = crate::document::state::MAX_DEPTH;
                if v.depth > limit
                    && matches!(
                        v.handle().data,
                        NodeData::Text { .. } | NodeData::Element { .. }
                    )
                {
                    tree.doc
                        .traversal
                        .limit(TraversalKind::BoxBuild, limit, v.depth);
                    continue;
                }
                match &v.handle().data {
                    NodeData::Text { contents } => super::text_nodes::walk_text(
                        contents.borrow().to_string(),
                        v.preserve,
                        &v.survey.nodes,
                        v.index,
                        output.last_mut().expect("output"),
                    ),
                    NodeData::Element { .. } => {
                        if let Some(mut p) = enter(
                            v,
                            tree,
                            counter,
                            counters,
                            path,
                            output.last_mut().expect("output"),
                        ) {
                            let (children, restore) = descend(&p, tree, path);
                            output.push(std::mem::take(&mut p.children));
                            work.push(Work::Leave(p, restore));
                            work.extend(children.into_iter().rev().map(Work::Visit));
                        }
                    }
                    _ => {
                        let jobs = visits(
                            Survey::children(v.handle(), tree),
                            v.scope.clone(),
                            v.vars.clone(),
                            v.preserve,
                            v.depth,
                            v.vcx,
                        );
                        work.extend(jobs.into_iter().rev().map(Work::Visit));
                    }
                }
            }
        }
    }
    *out = output.pop().expect("root output");
}
