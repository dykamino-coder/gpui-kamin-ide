//! Обход DOM с операциями входа/выхода; глубина не расходует стек потока.
use super::{
    descent::{descend, visits},
    enter::enter,
    leave::leave,
    pending::{Pending, Restore},
    sibling_survey::{Survey, Visit},
};
use crate::budget::admission::Buffer;
use crate::dom::outcome::TraversalKind;
use crate::dom::{Node, shadow::Scope, tree_index::TreeIndex};
use crate::style::{css::VarMap, generated::counters::Counters, select::Ancestor};
use crate::values::ValueCx;
use markup5ever_rcdom::{Handle, NodeData};
use std::rc::Rc;

enum Work<'a> {
    Visit(Visit<'a>),
    Leave(Box<Pending<'a>>, Restore, Option<super::record::Start>),
}
#[allow(clippy::too_many_arguments)]
pub(in crate::dom) fn walk(
    handle: &Handle,
    tree: &TreeIndex,
    scope: &Rc<Scope>,
    vars: &VarMap,
    counters: &mut Counters,
    path: &mut Vec<Ancestor>,
    preserve: bool,
    out: &mut Vec<Node>,
    vcx: ValueCx<'_>,
    mut invalidation: Option<&mut crate::dom::invalidation::Invalidation>,
    mut recorder: Option<&mut super::record::Recorder<'_, '_>>,
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
    output.push(super::kids::Kids::from_vec(std::mem::take(out)), &admission)?;
    while let Some(next) = work.pop() {
        admission.work(1)?;
        match next {
            Work::Leave(p, restore, start) => {
                restore.apply(path);
                let children = output.pop().expect("children output").into_vec();
                leave(
                    p,
                    path,
                    counters,
                    children,
                    output.last_mut().expect("parent output"),
                    invalidation.as_deref_mut(),
                )?;
                if let (Some(recorder), Some(start)) = (&mut recorder, start) {
                    recorder.finish(
                        start,
                        path,
                        counters,
                        output.last().expect("parent output"),
                        &admission,
                        vcx,
                    )?;
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
                        let start = recorder
                            .as_deref_mut()
                            .map(|r| {
                                r.begin(&v, path, counters, output.last().expect("output").len())
                            })
                            .transpose()?;
                        if let (Some(recorder), Some(start)) = (&mut recorder, &start)
                            && recorder.reuse(
                                start,
                                path,
                                counters,
                                output.last_mut().expect("output"),
                                v.vcx,
                            )?
                        {
                            continue;
                        }
                        if let Some(mut p) = enter(
                            v,
                            tree,
                            &sharing,
                            counters,
                            path,
                            output.last_mut().expect("output"),
                        ) {
                            if let (Some(recorder), Some(start)) = (&mut recorder, &start) {
                                recorder.entered(&p, start)?;
                            }
                            let (mut children, restore, sequence) = descend(&p, tree, path)?;
                            if let Some(recorder) = &mut recorder {
                                recorder.sequence(sequence);
                            }
                            output.push(std::mem::take(&mut p.children), &admission)?;
                            work.push(Work::Leave(p, restore, start), &admission)?;
                            while let Some(job) = children.pop() {
                                work.push(Work::Visit(job), &admission)?;
                            }
                        } else if let (Some(recorder), Some(start)) = (&mut recorder, start) {
                            recorder.finish(
                                start,
                                path,
                                counters,
                                output.last().expect("output"),
                                &admission,
                                vcx,
                            )?;
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
