//! The common dirty-closure consumer stages raw cascade before normalization and commit.
use super::{CascadeEntry, MemoStage, RootOutput};
use crate::budget::admission::Error;
use crate::dom::{shadow::Scope, tree_index::TreeIndex};
use crate::style::{css::VarMap, generated::counters::Counters};
use crate::values::ValueCx;
use markup5ever_rcdom::Handle;
use std::rc::Rc;

/// The document source root and current prepared rules/matching tables, without reparsing CSS.
/// Add complete selector/generated-content invalidation to the stage before walking.
pub struct CascadeInput<'a> {
    pub source: &'a Handle,
    pub tree: &'a TreeIndex,
    pub scope: &'a Rc<Scope>,
    pub vars: &'a VarMap,
    pub values: ValueCx<'a>,
}

/// The values/bindings transaction boundary. `true` admits saved values under the
/// current ValueCx and certifies stable TextId, element/read bindings for the segment.
/// `false` changes nothing and continues ordinary cascade; an error aborts the stage.
/// Implementations must neither walk descendants nor retain a previous DocumentValues.
pub trait ResumeBindings {
    fn resume(&mut self, entry: &CascadeEntry) -> Result<bool, Error>;
}

impl MemoStage<'_> {
    /// Returns raw output only: the caller normalizes it with current contexts, then
    /// atomically publishes bindings, BoxDelta and this stage. Error/abort leaves the
    /// committed memo intact; ValueCx and bindings belong to an abortable transaction.
    pub fn cascade(
        &mut self,
        input: CascadeInput<'_>,
        bindings: &mut dyn ResumeBindings,
    ) -> Result<RootOutput, Error> {
        let mut recorder = crate::dom::walk::Recorder::retained(self, bindings);
        let mut out = Vec::new();
        crate::dom::walk::walk(
            input.source,
            input.tree,
            input.scope,
            input.vars,
            &mut Counters::default(),
            &mut Vec::new(),
            false,
            &mut out,
            input.values,
            None,
            Some(&mut recorder),
        )?;
        let (update, output) = recorder.finish_update(input.source.key(), &out)?;
        self.update.merge(update)?;
        Ok(output)
    }
}
