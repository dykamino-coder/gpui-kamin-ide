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

type Table = RefCell<HashMap<(PaintSegmentId, u64), PaintRecord>>;

/// Own records of continued roots, by destination segment and root node.
#[derive(Clone, Default)]
pub(crate) struct BareRecords(Rc<Table>);

impl BareRecords {
    fn table(&self) -> &Table {
        &self.0
    }

    pub(crate) fn insert(&self, segment: PaintSegmentId, root: u64, record: PaintRecord) {
        let table: &RefCell<HashMap<(PaintSegmentId, u64), PaintRecord>> = self.table();
        table.borrow_mut().insert((segment, root), record);
    }

    pub(crate) fn get(&self, segment: PaintSegmentId, root: u64) -> Option<PaintRecord> {
        self.table().borrow().get(&(segment, root)).cloned()
    }

    /// Move every record of other into this table.
    pub(crate) fn absorb(&self, other: &BareRecords) {
        let table: &RefCell<HashMap<(PaintSegmentId, u64), PaintRecord>> = self.table();
        let other: &RefCell<HashMap<(PaintSegmentId, u64), PaintRecord>> = other.table();
        let moved = std::mem::take(&mut *other.borrow_mut());
        table.borrow_mut().extend(moved);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.table().borrow().is_empty()
    }
}
