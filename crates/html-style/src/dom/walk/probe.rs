//! Restyle probe: the cascade of a few target elements through the walk's own
//! entry (`enter`), without building boxes for the rest of the document.
//!
//! A batch of class mutations restyles only the elements its invalidation
//! finds (`dom::invalidation::targets`). Blink recalculates exactly those
//! (`StyleInvalidator::Invalidate` marks them, `style_engine.cc`
//! `RecalcStyle` visits only marked subtrees); Stylo restyles from the
//! dirty roots down (`traversal.rs`). Here the walk descends only the
//! ancestor chain of each target, so every target is cascaded in its real
//! context (ancestor path for matching, inherited custom properties, scopes)
//! by the same code as a full build, and stops at the target: its subtree
//! inherits by rematerialization (`style::cascade::materialize`), not by a
//! second cascade.

use super::engine::{descend, visits};
use super::enter::enter;
use super::pending::Restore;
use super::sibling_survey::{Survey, Visit};
use crate::budget::admission::{Buffer, Error};
use crate::dom::shadow::Scope;
use crate::dom::tree_index::TreeIndex;
use crate::style::computed::Computed;
use crate::style::css::VarMap;
use crate::values::ValueCx;
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashSet;
use std::rc::Rc;

/// What the walk's entry produced for one target element.
#[derive(Debug)]
pub struct Probed {
    /// The cascaded style of the element as the walk leaves its entry.
    pub style: Box<Computed>,
    /// Everything else the entry derived from the cascade: equal in both
    /// probes, or the batch reaches past the element's own style.
    pub rest: ProbedRest,
}

/// The entry's outputs beside the element's own style.
#[derive(Debug, PartialEq)]
pub struct ProbedRest {
    pub vars: Rc<VarMap>,
    pub hover: Option<Box<Computed>>,
    pub first_letter: Option<Box<Computed>>,
    pub first_line: Option<Box<Computed>>,
    pub list_item: Option<i64>,
    pub hidden: bool,
    pub box_level: bool,
    /// The element generates a `::before` box.
    pub before: bool,
    pub shadow: bool,
}

enum Work<'a> {
    Visit(Visit<'a>),
    Leave(Restore),
}

/// The entries of `targets` (element handles of the document under `root`),
/// in their order; `None` for a target the walk does not enter (dropped tag,
/// `display: none` subtree, inside a hidden or shadow-hosting ancestor).
pub fn probe(
    root: &Handle,
    tree: &TreeIndex,
    scope: &Rc<Scope>,
    targets: &[Handle],
    vcx: ValueCx<'_>,
) -> Result<Vec<Option<Probed>>, Error> {
    let admission = tree.admission();
    let address = |h: &Handle| Rc::as_ptr(h) as usize;
    let wanted: Vec<usize> = targets.iter().map(address).collect();
    let mut chain = HashSet::new();
    for target in targets {
        let mut at = parent(target);
        while let Some(node) = at {
            if !chain.insert(address(&node)) {
                break;
            }
            at = parent(&node);
        }
    }
    let mut found: Vec<Option<Probed>> = targets.iter().map(|_| None).collect();
    let sharing = super::element_sharing::ElementSharing::default();
    let mut counter = 0u64;
    let mut counters = crate::style::generated::counters::Counters::default();
    let mut path = Vec::new();
    let mut scratch = super::kids::Kids::default();
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
                let Some(p) = enter(
                    v,
                    tree,
                    &sharing,
                    &mut counter,
                    &mut counters,
                    &path,
                    &mut scratch,
                ) else {
                    continue;
                };
                if let Some(at) = target {
                    found[at] = Some(Probed {
                        style: p.style.clone(),
                        rest: ProbedRest {
                            vars: p.vars.clone(),
                            hover: p.hover.clone(),
                            first_letter: p.first_letter.clone(),
                            first_line: p.first_line.clone(),
                            list_item: p.list_item,
                            hidden: p.hidden,
                            box_level: p.box_level,
                            before: !p.children.is_empty(),
                            shadow: p.shadow.is_some(),
                        },
                    });
                }
                if on_chain {
                    let (children, restore) = descend(&p, tree, &mut path)?;
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
    Ok(found)
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

fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}
