//! Фрагментация: неизменяемая цепочка измерений родительских строк.

use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct FragmentationContext {
    /// Immutable parent frames of this fragmentation measurement.
    pub(crate) lines: Option<Rc<crate::layout::fragment::LineCx>>,
    /// Row extents of the spanner flow being built (`RowSlot`).
    pub(crate) spanner_rows: Option<crate::layout::multicol::spanner::RowSlot>,
}
