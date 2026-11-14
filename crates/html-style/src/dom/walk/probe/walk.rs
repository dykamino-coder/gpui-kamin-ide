//! The probe's walk down the ancestor chains of the targets no kept context
//! reaches (`Chains`): the build's own entries and descents, for the chains
//! only.

use super::super::engine::{descend, keeps, visits};
use super::super::enter::enter;
use super::super::pending::Restore;
use super::super::sibling_survey::{Survey, Visit};
use super::{Chains, Probed, Run, parent};
use crate::budget::admission::{Buffer, Error};
use crate::dom::shadow::Scope;
use crate::dom::tree_index::TreeIndex;
use crate::style::css::VarMap;
use crate::values::ValueCx;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashSet;
use std::rc::Rc;
enum Work<'a> {
    Visit(Visit<'a>),
    Leave(Restore),
}

/// Enter the targets `cold` of `run` down their ancestor chains from `root`,
/// and any other target met on the way; `chains` keeps the contexts of the
/// parents of targets.
pub(super) fn walk<'a>(
    root: &Handle,
    tree: &TreeIndex,
    scope: &Rc<Scope>,
    cold: &[usize],
    mut chains: Option<&mut Chains>,
    run: &mut Run<'_>,
    vcx: ValueCx<'a>,
) -> Result<(), Error> {
    let admission = tree.admission();
    let address = |h: &Handle| Rc::as_ptr(h) as usize;
    let wanted: Vec<usize> = run.targets.iter().map(address).collect();
    let parents: HashSet<usize> = run.parents.iter().flatten().map(address).collect();
    let mut chain = HashSet::new();
    for &i in cold {
        let mut at = run.parents[i].clone();
        while let Some(node) = at {
            if !chain.insert(address(&node)) {
                break;
            }
            at = parent(&node);
        }
    }
    let mut path = Vec::new();
    let mut work: Buffer<Work<'_>> = Buffer::default();
    let start = visits(
        Survey::children(root, tree),
        scope.clone(),
        Rc::new(VarMap::new()),
        false,
        1,
        vcx,
        0,
    )?;
    push_visits(start, &mut work, &admission)?;
    while let Some(next) = work.pop() {
        admission.work(1)?;
        let v = match next {
            Work::Leave(restore) => {
                restore.apply(&mut path);
                continue;
            }
            Work::Visit(v) => v,
        };
        let here = address(v.handle());
        let on_chain = chain.contains(&here);
        match &v.handle().data {
            NodeData::Element { .. } => {
                let target = wanted.iter().position(|&w| w == here);
                if target.is_none() && !on_chain {
                    continue;
                }
                if let Some(at) = target {
                    run.done[at] = true;
                }
                let Some(p) = enter(
                    v,
                    tree,
                    &run.sharing,
                    &mut run.counter,
                    &mut run.counters,
                    &path,
                    &mut run.scratch,
                ) else {
                    continue;
                };
                if let Some(at) = target {
                    run.found[at] = Some(Probed::of(&p));
                    if let Some(chains) = chains.as_deref_mut() {
                        chains.entered(path.len(), &p.me, &p.vars, keeps(&p));
                    }
                }
                if on_chain {
                    let above = path.len();
                    let (mut children, restore) = descend(&p, tree, &mut path)?;
                    // The plain descent: the element's own children under
                    // its record (not a shadow tree or a slot's assignment).
                    let plain = matches!(restore, Restore::Truncate(_)) && path.len() == above + 1;
                    if plain
                        && parents.contains(&here)
                        && let Some(chains) = chains.as_deref_mut()
                        && let Some(first) = children.pop()
                    {
                        chains.capture(p.visit.handle(), &path, &first);
                        children.push(first, &admission)?;
                    }
                    work.push(Work::Leave(restore), &admission)?;
                    push_visits(children, &mut work, &admission)?;
                }
            }
            NodeData::Text { .. } => {}
            _ if on_chain => {
                let jobs = visits(
                    Survey::children(v.handle(), tree),
                    v.scope.clone(),
                    v.vars.clone(),
                    v.preserve,
                    v.depth,
                    v.vcx,
                    0,
                )?;
                push_visits(jobs, &mut work, &admission)?;
            }
            _ => {}
        }
    }
    for &i in cold {
        run.done[i] = true;
    }
    Ok(())
}
fn push_visits<'a>(
    mut jobs: Buffer<Visit<'a>>,
    work: &mut Buffer<Work<'a>>,
    admission: &crate::budget::admission::Admission,
) -> Result<(), Error> {
    while let Some(job) = jobs.pop() {
        work.push(Work::Visit(job), admission)?;
    }
    Ok(())
}
