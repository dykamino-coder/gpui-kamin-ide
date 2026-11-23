//! A clean segment admits bindings before restoring effects and appending raw nodes.
use super::kids::Kids;
use crate::budget::admission::Error;
use crate::dom::NodeKey;
use crate::dom::source_memo::{EntryInputs, MemoStage, ResumeBindings};
use crate::style::{generated::counters::Counters, select::Ancestor};
use crate::values::ValueCx;

pub(super) struct Resume<'a, 'm> {
    pub stage: &'a MemoStage<'m>,
    pub bindings: &'a mut dyn ResumeBindings,
}

impl Resume<'_, '_> {
    pub fn append(
        &mut self,
        node: NodeKey,
        inputs: &EntryInputs,
        path: &[Ancestor],
        counters: &mut Counters,
        out: &mut Kids,
        vcx: ValueCx<'_>,
    ) -> Result<bool, Error> {
        let Some(entry) = self.stage.reusable(node, inputs) else {
            return Ok(false);
        };
        let group = path.iter().rev().find_map(|ancestor| {
            ancestor
                .target_group
                .as_ref()
                .map(|selected| (ancestor.key.handle().key(), selected.as_ref()))
        });
        if entry
            .effects
            .selects_group
            .is_some_and(|expected| group.is_none_or(|(actual, _)| actual != expected))
        {
            return Ok(false);
        }
        out.try_reserve(entry.raw.len())
            .map_err(|_| Error::Allocation)?;
        if !self.bindings.resume(entry)? {
            return Ok(false);
        }
        let restored = entry.effects.restore(vcx, counters, group);
        debug_assert!(restored, "group was checked before admitting bindings");
        out.extend(entry.raw.iter().cloned());
        Ok(true)
    }
}
