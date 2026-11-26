//! A materialized style and the inputs it was computed under.

use super::inherited;
use crate::style::computed::Computed;
use crate::style::publication::Published;
use crate::values::ValueCx;
use std::rc::Rc;

pub(super) type Inputs = (u64, [u32; 2]);

/// The inputs a materialized style was computed under: the font generation,
/// and the viewport size only if its values read it (`viewport_reads`).
pub(super) type Stored = (u64, Option<[u32; 2]>);

/// What a computation under `generation` keeps of it.
pub(super) fn stored(generation: Inputs, read_viewport: bool) -> Stored {
    (generation.0, read_viewport.then_some(generation.1))
}

pub(super) fn inputs(vcx: ValueCx<'_>) -> Inputs {
    (
        vcx.doc.font_generation(),
        [vcx.viewport.0.to_bits(), vcx.viewport.1.to_bits()],
    )
}

pub(super) struct Materialized {
    /// The parent's computed style it was computed under.
    pub(super) parent: Rc<Computed>,
    /// Generation of the font inputs and the viewport size for relative units,
    /// if the values read them.
    pub(super) generation: Stored,
    pub(super) computed: Published,
}

impl Materialized {
    /// Whether the style holds under the parent `parent`: the same inherited data. A style
    /// with a cascaded `inherit` of a non-inherited property (`explicit`) also depends on
    /// the parent's non-inherited values (css-cascade-5 §7.3.1): only the same parent
    /// or one equal in everything fits it (Blink `MatchedPropertiesCache`: explicit inheritance
    /// also checks the parent's non-inherited data).
    pub(super) fn fits(&self, parent: &Computed, generation: Inputs, own: &Computed) -> bool {
        self.generation.0 == generation.0
            && self
                .generation
                .1
                .is_none_or(|viewport| viewport == generation.1)
            && inherited::parent_eq(&self.parent, parent, own)
    }
}
