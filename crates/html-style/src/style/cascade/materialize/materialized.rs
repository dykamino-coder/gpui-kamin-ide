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
    /// Вычисленный стиль родителя, под которым он посчитан.
    pub(super) parent: Rc<Computed>,
    /// Поколение шрифтовых входов и размеры viewport для относительных единиц,
    /// если значения их прочитали.
    pub(super) generation: Stored,
    pub(super) computed: Published,
}

impl Materialized {
    /// Годится ли стиль под родителем `parent`: те же унаследованные данные. Стиль с
    /// каскадным `inherit` ненаследуемого свойства (`explicit`) зависит и от
    /// ненаследуемых значений родителя (css-cascade-5 §7.3.1): ему годен только тот же
    /// родитель или равный во всём (Blink `MatchedPropertiesCache`: explicit inheritance
    /// сверяет и non-inherited данные родителя).
    pub(super) fn fits(&self, parent: &Computed, generation: Inputs, own: &Computed) -> bool {
        self.generation.0 == generation.0
            && self
                .generation
                .1
                .is_none_or(|viewport| viewport == generation.1)
            && inherited::parent_eq(&self.parent, parent, own)
    }
}
