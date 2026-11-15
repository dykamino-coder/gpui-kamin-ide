//! A restyle of the cascaded stage in place (`Document` class mutations):
//! which styles take an ancestor's new inherited values by rematerialization
//! alone, and the edit of the paint-only groups.

use super::SharedStyle;
use crate::style::computed::Computed;

impl SharedStyle {
    /// Whether this style keeps values copied from computed styles outside
    /// the materialization (`style::cascade::materialize`): layout's used
    /// copy, the box of a moved element's tree parent, a `position-try`
    /// source, or decoration inputs resolved by document preparation. Such a
    /// style does not follow a new inherited value of an ancestor by
    /// rematerialization.
    pub fn holds_inherited_copies(&self) -> bool {
        self.used.is_some()
            || self
                .box_facts
                .as_ref()
                .is_some_and(|f| f.moved_from.is_some() || f.try_source.is_some())
            || self
                .decoration
                .as_ref()
                .is_some_and(|d| d.paint.is_some() || d.inline != Default::default())
    }

    /// Whether the computed stage of this style reads non-inherited values of
    /// its parent (an explicit `inherit` of a reset property): after a change
    /// of such a value it follows the parent only by rematerialization.
    pub fn inherits_reset(&self) -> bool {
        crate::style::cascade::materialize::inherits_reset(&self.cascaded)
    }

    /// Whether document preparation resolved decoration inputs of this box
    /// from cascaded values (the canvas background and its donor,
    /// css-backgrounds-3 §2.11.2): a new cascaded background does not reach
    /// them in place.
    pub fn holds_decoration(&self) -> bool {
        self.decoration.is_some()
    }

    /// Replace the paint-only groups of the cascaded stage (the inherited UI
    /// group and the box's own background, `Computed::with_paint_of`) by
    /// those of `new`; the computed stage materializes again under the same
    /// parent.
    pub fn set_cascaded_paint(&mut self, new: &Computed) {
        let cascaded = self.cascaded_mut();
        *cascaded = cascaded.with_paint_of(new);
    }
}
