//! A restyle of the cascaded stage in place (`Document` class mutations):
//! which styles take an ancestor's new inherited values by rematerialization
//! alone, and the edit of the inherited UI group.

use super::SharedStyle;
use crate::style::computed::{InheritedUi, StyleGroup};

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

    /// Replace the inherited UI group of the cascaded stage; the computed
    /// stage materializes again under the same parent.
    pub fn set_cascaded_ui(&mut self, ui: StyleGroup<InheritedUi>) {
        let cascaded = self.cascaded_mut();
        std::rc::Rc::make_mut(&mut cascaded.inh.0).ui = ui;
    }
}
