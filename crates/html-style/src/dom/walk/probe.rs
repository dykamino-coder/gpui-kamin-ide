//! Restyle probe: the cascade of a few target elements through the walk's own
//! entry (`enter`), without building boxes for the rest of the document.
//!
//! A batch of attribute mutations restyles only the elements its invalidation
//! finds (`dom::invalidation::targets`). Blink recalculates exactly those
//! (`StyleInvalidator::Invalidate` marks them, `style_engine.cc`
//! `RecalcStyle` visits only marked subtrees) under the styles their parents
//! already have; Stylo restyles from the dirty roots down (`traversal.rs`).
//! Here every target is cascaded in its real context (ancestor path for
//! matching, inherited custom properties, scopes) by the same code as a full
//! build. The first probe below a parent walks the ancestor chain once and
//! keeps the context its children are entered with (`Chains`); later probes
//! enter the target alone. The walk stops at the target: its subtree
//! inherits by rematerialization (`style::cascade::materialize`), not by a
//! second cascade.

mod chains;
mod walk;
pub use chains::Chains;
use walk::walk;

use super::engine::keeps;
use super::enter::enter;
use super::pending::Pending;
use crate::budget::admission::Error;
use crate::dom::shadow::Scope;
use crate::dom::tree_index::TreeIndex;
use crate::style::computed::Computed;
use crate::style::css::VarMap;
use crate::values::ValueCx;
use markup5ever_rcdom::Handle;
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

impl Probed {
    fn of(p: &Pending<'_>) -> Self {
        Self {
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
        }
    }
}

/// The state of one probe: its targets and what the entries share.
struct Run<'t> {
    targets: &'t [Handle],
    parents: Vec<Option<Handle>>,
    found: Vec<Option<Probed>>,
    /// The target was entered, or the walk passed it by.
    done: Vec<bool>,
    sharing: super::element_sharing::ElementSharing,
    counter: u64,
    counters: crate::style::generated::counters::Counters,
    scratch: super::kids::Kids,
}

/// The entries of `targets` (element handles of the document under `root`),
/// in their order; `None` for a target the walk does not enter (dropped tag,
/// `display: none` subtree, inside a hidden or shadow-hosting ancestor).
/// `chains` keeps the contexts of the parents between the probes of one
/// document.
pub fn probe(
    root: &Handle,
    tree: &TreeIndex,
    scope: &Rc<Scope>,
    targets: &[Handle],
    chains: &mut Chains,
    vcx: ValueCx<'_>,
) -> Result<Vec<Option<Probed>>, Error> {
    chains.root(root, tree);
    let mut run = Run {
        targets,
        parents: targets.iter().map(parent).collect(),
        found: targets.iter().map(|_| None).collect(),
        done: vec![false; targets.len()],
        sharing: Default::default(),
        counter: 0,
        counters: Default::default(),
        scratch: Default::default(),
    };
    let known = |i: usize, run: &Run<'_>, chains: &Chains| {
        run.parents[i].as_ref().is_some_and(|p| chains.knows(p))
    };
    let cold: Vec<usize> = (0..targets.len())
        .filter(|&i| !known(i, &run, chains))
        .collect();
    if !cold.is_empty() {
        walk(root, tree, scope, &cold, Some(chains), &mut run, vcx)?;
        // A target the walk gave a context enters again under it: both
        // probes of a batch then enter it alike (the counters of an entry
        // under a context do not pass through its ancestors).
        for i in 0..targets.len() {
            if known(i, &run, chains) {
                run.done[i] = false;
                run.found[i] = None;
            }
        }
        run.counters = Default::default();
    }
    // Targets enter under the kept contexts, ancestors first: a target on
    // the path of another gives it its new record (`Chains::entered`).
    let mut warm: Vec<(usize, usize)> = (0..targets.len())
        .filter(|&i| !run.done[i])
        .map(|i| (depth(&targets[i]), i))
        .collect();
    warm.sort_unstable();
    let admission = tree.admission();
    let mut left = Vec::new();
    for (_, i) in warm {
        admission.work(1)?;
        let entry = run.parents[i]
            .as_ref()
            .and_then(|p| chains.visit(p, &targets[i], scope, vcx));
        // The walk above made room for its own contexts.
        let Some((visit, path)) = entry else {
            left.push(i);
            continue;
        };
        let above = path.len();
        let entered = enter(
            visit,
            tree,
            &run.sharing,
            &mut run.counter,
            &mut run.counters,
            path,
            &mut run.scratch,
        );
        run.done[i] = true;
        if let Some(p) = entered {
            run.found[i] = Some(Probed::of(&p));
            chains.entered(above, &p.me, &p.vars, keeps(&p));
        }
    }
    if !left.is_empty() {
        walk(root, tree, scope, &left, None, &mut run, vcx)?;
    }
    Ok(run.found)
}

fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}

/// The number of ancestors of `node` up to the document node.
fn depth(node: &Handle) -> usize {
    let mut depth = 0;
    let mut at = parent(node);
    while let Some(node) = at {
        depth += 1;
        at = parent(&node);
    }
    depth
}
