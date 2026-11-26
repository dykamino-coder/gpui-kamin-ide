//! The boxes of one element's subtree, built by the walk of a full build
//! under the context the element is entered with (`Chains`): the rebuild of
//! a subtree whose boxes an edit changes (`Damage::Rebox`).
//!
//! Blink rebuilds the layout tree of an element that needs reattachment
//! under its parent's layout object (`Element::RebuildLayoutTree`,
//! `LayoutTreeBuilderForElement`), not the document's; Gecko recreates the
//! frames of the content (`RestyleManager::RecreateFramesForContent`). Here
//! the element is entered as the probe enters a target (`walk::probe`), and
//! the walk then descends into its whole subtree with the build's own
//! entries, descents and exits. Its counters start empty and record what
//! the subtree reads from and hands to the document's (`Counters::isolated`).

use super::engine::run;
use super::probe::{Chains, probe};
use crate::budget::admission::{Buffer, Error};
use crate::dom::Node;
use crate::dom::shadow::Scope;
use crate::dom::tree_index::TreeIndex;
use crate::style::generated::counters::{Counters, Outside};
use crate::values::ValueCx;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

/// The boxes the walk built for one element and its subtree.
#[derive(Debug)]
pub struct Subtree {
    /// The nodes the element adds to its parent's children (one box, none
    /// for an element the walk drops, its children's for `display: contents`).
    pub nodes: Vec<Node>,
    /// What the subtree exchanged with the counters of the document.
    pub outside: Outside,
}

/// The subtree of `target` (an element of the document `root`) at the
/// current state of the DOM; `None` when the walk does not enter its parent
/// (the target is not in the document, or inside a hidden or shadow-hosting
/// ancestor).
pub fn subtree(
    root: &Handle,
    tree: &TreeIndex,
    scope: &Rc<Scope>,
    target: &Handle,
    chains: &mut Chains,
    vcx: ValueCx<'_>,
) -> Result<Option<Subtree>, Error> {
    let Some(parent) = parent(target) else {
        return Ok(None);
    };
    if !chains.knows(&parent) {
        // The probe's walk down the chain keeps the parent's context.
        probe(root, tree, scope, std::slice::from_ref(target), chains, vcx)?;
    }
    let Some((visit, path)) = chains.visit(&parent, target, scope, vcx) else {
        return Ok(None);
    };
    let mut path = path.to_vec();
    let mut jobs = Buffer::default();
    jobs.push(visit, &tree.admission())?;
    let mut counters = Counters::isolated();
    let mut nodes = Vec::new();
    run(jobs, tree, &mut counters, &mut path, &mut nodes, None)?;
    let outside = counters.outside().unwrap_or_default();
    Ok(Some(Subtree { nodes, outside }))
}

/// The box of `target` alone, with the boxes its own entry and exit generate
/// (`::before`, `::after`, `::marker`) and none of its children: what the
/// element's attributes reach through `attr()` in generated content.
pub fn shallow(
    root: &Handle,
    tree: &TreeIndex,
    scope: &Rc<Scope>,
    target: &Handle,
    chains: &mut Chains,
    vcx: ValueCx<'_>,
) -> Result<Option<Vec<Node>>, Error> {
    let Some(parent) = parent(target) else {
        return Ok(None);
    };
    if !chains.knows(&parent) {
        probe(root, tree, scope, std::slice::from_ref(target), chains, vcx)?;
    }
    let Some((visit, path)) = chains.visit(&parent, target, scope, vcx) else {
        return Ok(None);
    };
    let path = path.to_vec();
    let sharing = super::element_sharing::ElementSharing::default();
    let mut counters = Counters::isolated();
    let mut out = super::kids::Kids::default();
    if let Some(mut p) = super::enter::enter(visit, tree, &sharing, &mut counters, &path, &mut out)
    {
        let children = std::mem::take(&mut p.children).into_vec();
        super::leave::leave(p, &path, &mut counters, children, &mut out, None)?;
    }
    Ok(Some(out.into_vec()))
}

fn parent(node: &Handle) -> Option<Handle> {
    let weak = node.parent.take();
    let parent = weak
        .as_ref()
        .and_then(markup5ever_rcdom::WeakHandle::upgrade);
    node.parent.set(weak);
    parent
}
