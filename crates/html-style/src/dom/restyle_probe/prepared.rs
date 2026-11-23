//! Current matching state shared by targeted probes and the retained cascade walk.
use super::{RestyleCache, RestyleProbe, scheme};
use crate::dom::{
    input::Admitted, outcome::DocumentFailure, shadow::Scope, source_memo::CascadeInput,
    tree_index::TreeIndex,
};
use crate::style::{css::VarMap, select::has::mark_has};
use crate::values::{DocumentValues, ValueCx};
use markup5ever_rcdom::Handle;
use std::rc::Rc;

/// A revision-local index, never the previous revision's matching or slot caches.
/// Rules must already be valid for the current values/resources; this does not
/// admit saved values or bindings across owners (that is ResumeBindings' job).
pub struct PreparedCascade {
    pub(super) tree: TreeIndex,
    pub(super) scope: Rc<Scope>,
    pub(super) source: Handle,
    vars: VarMap,
    media: crate::style::css::Media,
}

impl PreparedCascade {
    pub fn input(&self) -> CascadeInput<'_> {
        CascadeInput {
            source: &self.source,
            tree: &self.tree,
            scope: &self.scope,
            vars: &self.vars,
            values: self.values(),
        }
    }

    pub(super) fn values(&self) -> ValueCx<'_> {
        self.tree.doc.values().with_scheme(scheme(self.media))
    }
}

impl RestyleCache {
    /// Prepare current matching after selector invalidation has discarded stale
    /// :has marks. The caller must invalidate all affected memo entries as well.
    /// Like probe(), this supports document rules without declarative shadows;
    /// shadows require their current scope/slot preparation before entering walk.
    pub fn prepare_cascade(
        &mut self,
        probe: &RestyleProbe,
        input: &Admitted,
        values: &Rc<DocumentValues>,
    ) -> Result<PreparedCascade, DocumentFailure> {
        let mut tree = TreeIndex::new(values.clone(), input.xhtml);
        if !probe.has_args.is_empty() {
            match self.has.take() {
                Some(has) => tree.put_has(has),
                None => mark_has(
                    &input.dom.document,
                    &probe.has_args,
                    &mut Vec::new(),
                    &mut tree,
                )?,
            }
        }
        Ok(PreparedCascade {
            tree,
            scope: probe.doc.clone(),
            source: input.dom.document.clone(),
            vars: VarMap::new(),
            media: probe.media,
        })
    }
}
