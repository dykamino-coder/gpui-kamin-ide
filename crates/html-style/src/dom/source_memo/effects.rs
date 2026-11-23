//! Detached traversal outputs restored when a clean raw segment is reused.
use super::NodeKey;
use crate::style::cascade::sharing::Environment;
use crate::style::generated::counters::Counters;
use crate::style::values::value::RootMetrics;
use crate::values::ValueCx;
use std::cell::Cell;

#[derive(Clone)]
pub struct CascadeEffects {
    pub counters: Counters,
    pub root: Option<RootMetrics>,
    pub selects_group: Option<NodeKey>,
}

impl CascadeEffects {
    pub(crate) fn capture(
        before: &Environment,
        vcx: ValueCx<'_>,
        counters: &Counters,
        selects_group: Option<NodeKey>,
    ) -> Self {
        let root = vcx.doc.root.borrow();
        Self {
            counters: counters.clone(),
            root: (!before.root_matches(&root)).then(|| root.clone()),
            selects_group,
        }
    }

    pub(super) fn extra_bytes(&self) -> usize {
        self.root.as_ref().map_or(0, |root| root.family.capacity())
    }

    /// Validate the current group before publishing any traversal side effect.
    /// The cell belongs to this walk, never to the memo or a previous frame.
    pub fn restore(
        &self,
        vcx: ValueCx<'_>,
        counters: &mut Counters,
        current_group: Option<(NodeKey, &Cell<bool>)>,
    ) -> bool {
        let group = match self.selects_group {
            Some(expected) => match current_group {
                Some((actual, selected)) if actual == expected => Some(selected),
                _ => return false,
            },
            None => None,
        };
        if let Some(root) = &self.root {
            *vcx.doc.root.borrow_mut() = root.clone();
        }
        *counters = self.counters.clone();
        if let Some(selected) = group {
            selected.set(true);
        }
        true
    }
}
