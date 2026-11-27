//! The box stages remembered on a style (`style::cascade::materialize::stage_memo`).

use super::SharedStyle;
use crate::style::cascade::materialize::{OwnKey, StageMemo, identity};

impl SharedStyle {
    /// The remembered box stages of this style.
    pub(crate) fn stage_memo(&self) -> &StageMemo {
        &self.stage
    }

    /// The allocations of every stage a box stage reads: an edit of any gives a new one.
    pub(crate) fn stage_own(&self) -> OwnKey {
        let (materialized, container) = self.computed.stage_identity();
        OwnKey::new(
            [
                Some(identity(&self.cascaded)),
                materialized,
                self.used.as_ref().map(identity),
                self.box_facts.as_ref().map(identity),
                self.decoration.as_ref().map(identity),
            ],
            container,
        )
    }
}
