//! The used layer shared by like elements (`SharedStyle::derive_used`).

use super::SharedStyle;
use crate::style::computed::{Computed, DerivedGroups, ResetGroup};

impl SharedStyle {
    /// Edit reset group `T` of the used layer by edit `E` with `value` through
    /// the document's cache (`Computed::derive_used`). An element without a
    /// used layer takes the layer shared by every element of the same
    /// computed style and edit: thousands of like siblings hold one used
    /// style, not one copy each (Stylo shares computed values among like
    /// siblings, `style/sharing`; Blink `ComputedStyle` sharing).
    pub fn derive_used<E: 'static, T: Clone + 'static>(
        &mut self,
        derived: &DerivedGroups,
        value: u64,
        edit: impl FnOnce(&mut T),
    ) where
        Computed: ResetGroup<T>,
    {
        // The style module edits the layer (ADR3): an own used layer, or the only stage of
        // a style that was never materialized.
        if let Some(used) = &mut self.used {
            Computed::derive_used_in::<E, T>(used, derived, value, edit);
            return;
        }
        if self.computed.any().is_none() {
            Computed::derive_used_in::<E, T>(&mut self.cascaded, derived, value, edit);
            return;
        }
        let base = self.base();
        self.used = Some(
            derived.derive_shared::<(E, T), Computed>(&base, value, |used| {
                used.derive_used::<E, T>(derived, value, edit)
            }),
        );
    }
}
