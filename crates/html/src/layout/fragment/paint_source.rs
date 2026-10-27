//! Явный допуск исходных узлов к фрагментам потребителя и собственные записи
//! внепоточных корней, воспроизводимые в legacy-копиях их предков.

use super::decoration::Decoration;
use gpui::{Bounds, PaintRecord, PaintSegment, PaintSegmentId, PaintSegmentSlice, Pixels};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Admission of a native source paint: nodes and their fragment decorations,
/// node families, and the own capture of every continued absolute/fixed root.
#[derive(Clone, Default)]
pub(crate) struct SourceSlices {
    pub(crate) nodes: HashMap<u64, Rc<[PaintSegmentSlice]>>,
    pub(crate) boxes: HashMap<u64, Rc<[Decoration]>>,
    pub(crate) families: Rc<HashMap<u64, u64>>,
    pub(crate) nested: HashMap<u64, Rc<Nested>>,
}

/// Own continuation capture of an absolute/fixed root (CSS 2.1 Appendix E:
/// it paints in its stacking context, inside its ancestors' effect scopes).
/// Placements replayed by a legacy copy of the ancestors are translated to the
/// destination (`bare`); the others keep source coordinates and are
/// re-admitted by the enclosing source record, one effect scope per segment.
pub(crate) struct Nested {
    pub(crate) extent: Bounds<Pixels>,
    pub(crate) routes: Rc<[PaintSegment]>,
    pub(crate) slices: Rc<[PaintSegmentSlice]>,
    pub(crate) boxes: HashMap<u64, Rc<[Decoration]>>,
    pub(crate) bare: Rc<[PaintSegmentId]>,
}

/// Own records of continued roots, by destination segment and root node.
pub(crate) type BareRecords = Rc<RefCell<HashMap<(PaintSegmentId, u64), PaintRecord>>>;
