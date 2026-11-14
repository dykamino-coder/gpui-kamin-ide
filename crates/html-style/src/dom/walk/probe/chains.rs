//! Retained entry contexts of the parents of probed elements: what the walk
//! hands to the children of an element (the ancestor path, the sibling
//! census, inherited custom properties), kept between the probes of one
//! document so that a restyle enters its targets without entering their
//! ancestors again.
//!
//! Blink recalculates a dirty element under the computed style its parent
//! already has and matches its selectors by walking the DOM upwards
//! (`style_engine.cc` `RecalcStyle`, `selector_checker.cc`); no ancestor is
//! cascaded. Here the walk's own entry needs the ancestor records, so the
//! first probe below a parent walks the chain once and keeps its context.
//!
//! A context holds while the document is restyled in place (a rebuild drops
//! it with the document that held it), because of what such a batch may
//! change (`dom::invalidation::targets`):
//! - the cascade of an element that is not a target does not change, so its
//!   record keeps everything the cascade derived for it, and the record of a
//!   target keeps it too: its new style differs in paint values only;
//! - a mutated element is a target, and the record of a target on a kept
//!   path is replaced by the one its entry gives (`entered`): ancestor
//!   compounds read the attributes the batch left;
//! - a name that a rule reads in a sibling compound or in a selector argument
//!   never changes in place (such a batch is rebuilt), and the sheets of the
//!   document do not change, so a census taken before a batch still answers
//!   every question a selector asks the siblings.
use super::super::sibling_survey::{Survey, Visit};
use crate::dom::shadow::Scope;
use crate::dom::tree_index::TreeIndex;
use crate::style::css::VarMap;
use crate::style::select::Ancestor;
use crate::values::ValueCx;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

/// Contexts kept past this many are dropped together.
const LIMIT: usize = 16;

/// The contexts of one document.
#[derive(Default)]
pub struct Chains(Vec<Context>);

/// What the children of `parent` are entered with.
struct Context {
    /// The document node or an element; the handle keeps its address.
    parent: Handle,
    /// Records of the ancestors, `parent` last; empty under the document.
    path: Vec<Ancestor>,
    survey: Rc<Survey>,
    vars: Rc<VarMap>,
    preserve: bool,
}

impl Chains {
    fn find(&self, parent: &Handle) -> Option<&Context> {
        self.0.iter().find(|c| Rc::ptr_eq(&c.parent, parent))
    }

    /// The children of `parent` have a context.
    pub(super) fn knows(&self, parent: &Handle) -> bool {
        self.find(parent).is_some()
    }

    /// The walk descends into `parent` with `path` (its record last) and
    /// hands `first`, a visit of one of its children, to each child.
    pub(super) fn capture(&mut self, parent: &Handle, path: &[Ancestor], first: &Visit<'_>) {
        if self.knows(parent) {
            return;
        }
        if self.0.len() == LIMIT {
            self.0.clear();
        }
        self.0.push(Context {
            parent: parent.clone(),
            path: path.to_vec(),
            survey: first.survey.clone(),
            vars: first.vars.clone(),
            preserve: first.preserve,
        });
    }

    /// The children of the document node `root` are entered with no path.
    pub(super) fn root(&mut self, root: &Handle, tree: &TreeIndex) {
        if !self.knows(root) {
            self.0.push(Context {
                parent: root.clone(),
                path: Vec::new(),
                survey: Survey::children(root, tree),
                vars: Rc::new(VarMap::new()),
                preserve: false,
            });
        }
    }

    /// The visit of `target` under its parent's context and the path it is
    /// entered with; `None`: no context, or `target` is not a child.
    pub(super) fn visit<'a>(
        &self,
        parent: &Handle,
        target: &Handle,
        scope: &Rc<Scope>,
        vcx: ValueCx<'a>,
    ) -> Option<(Visit<'a>, &[Ancestor])> {
        let context = self.find(parent)?;
        let survey = &context.survey;
        let index = survey.nodes.iter().position(|n| Rc::ptr_eq(n, target))?;
        let pos = survey.spots[..index]
            .iter()
            .filter(|s| s.index != 0)
            .count();
        let visit = Visit {
            survey: survey.clone(),
            index,
            pos,
            scope: scope.clone(),
            vars: context.vars.clone(),
            preserve: context.preserve,
            depth: context.path.len() + 1,
            vcx,
            // Targets are few: each is cascaded, none shares a style.
            parent_share: 0,
        };
        Some((visit, &context.path))
    }

    /// The walk entered the element of record `me` below `above` ancestors
    /// and hands `vars` and `preserve` to its children: a path that holds
    /// the element with other attributes takes the new record.
    pub(super) fn entered(
        &mut self,
        above: usize,
        me: &Ancestor,
        vars: &Rc<VarMap>,
        preserve: bool,
    ) {
        for context in &mut self.0 {
            let Some(record) = context.path.get_mut(above) else {
                continue;
            };
            if record.key != me.key || record.attrs == me.attrs {
                continue;
            }
            *record = me.clone();
            if above + 1 == context.path.len() {
                context.vars = vars.clone();
                context.preserve = preserve;
            }
            // The records below carry the names of this one in their filters.
            for at in above + 1..context.path.len() {
                let (upper, lower) = context.path.split_at_mut(at);
                let bloom = upper[at - 1].ancestor_bloom.unwrap_or_default();
                let bloom = bloom.with_node(&lower[0]);
                lower[0].ancestor_bloom = Some(bloom);
            }
        }
    }
}
