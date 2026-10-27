//! Явный допуск исходных узлов к фрагментам потребителя и собственные записи
//! внепоточных корней, воспроизводимые в legacy-копиях их предков.

use super::decoration::Decoration;
use gpui::{
    Bounds, PaintHitScope, PaintRecord, PaintSegment, PaintSegmentId, PaintSegmentSlice, Pixels,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Admission of a native source paint: nodes and their fragment decorations,
/// node families, and the own capture of every continued absolute/fixed root.
#[derive(Clone, Default)]
pub(crate) struct SourceSlices {
    pub(crate) nodes: HashMap<u64, Rc<[PaintSegmentSlice]>>,
    pub(crate) boxes: HashMap<u64, Rc<[Decoration]>>,
    pub(crate) families: Rc<HashMap<u64, u64>>,
    pub(crate) nested: HashMap<u64, Rc<Nested>>,
    /// Placed regions re-admitted unmoved: the source record must reach them.
    pub(crate) placed: Option<Bounds<Pixels>>,
    /// Layout origin of every continued root the admission was built from;
    /// a root placed elsewhere at prepaint (a late static position) carries
    /// its whole family's admission along.
    pub(crate) origins: HashMap<u64, gpui::Point<Pixels>>,
}

/// Own continuation capture of an outermost absolute/fixed root and the flows
/// nested in it (CSS 2.1 Appendix E: it paints in its stacking context, inside
/// its ancestors' effect scopes). Every placement is translated to its
/// destination. `bare` segments are replayed by a legacy copy of the ancestors;
/// the others re-enter the source record unmoved, one effect scope per segment.
pub(crate) struct Nested {
    pub(crate) extent: Bounds<Pixels>,
    pub(crate) routes: Rc<[PaintSegment]>,
    /// Admission of every continued family inside the root's own capture.
    pub(crate) families: HashMap<u64, Rc<[PaintSegmentSlice]>>,
    pub(crate) boxes: HashMap<u64, Rc<[Decoration]>>,
    pub(crate) bare: Rc<[PaintSegmentId]>,
}

type Table = RefCell<HashMap<(PaintSegmentId, u64), Vec<PaintRecord>>>;

/// Own records of continued roots, by destination segment and root node. Every
/// native source that paints part of a root (a page-repeated layer of it, say)
/// adds its record; the slot replays them in source order.
#[derive(Clone, Default)]
pub(crate) struct BareRecords(Rc<Table>);

impl BareRecords {
    fn table(&self) -> &Table {
        &self.0
    }

    pub(crate) fn insert(&self, segment: PaintSegmentId, root: u64, record: PaintRecord) {
        let table: &RefCell<HashMap<(PaintSegmentId, u64), Vec<PaintRecord>>> = self.table();
        table
            .borrow_mut()
            .entry((segment, root))
            .or_default()
            .push(record);
    }

    pub(crate) fn get(&self, segment: PaintSegmentId, root: u64) -> Vec<PaintRecord> {
        self.table()
            .borrow()
            .get(&(segment, root))
            .cloned()
            .unwrap_or_default()
    }

    /// Move every record of other into this table, after its own.
    pub(crate) fn absorb(&self, other: &BareRecords) {
        let table: &RefCell<HashMap<(PaintSegmentId, u64), Vec<PaintRecord>>> = self.table();
        let other: &RefCell<HashMap<(PaintSegmentId, u64), Vec<PaintRecord>>> = other.table();
        let moved = std::mem::take(&mut *other.borrow_mut());
        let mut table = table.borrow_mut();
        for (key, records) in moved {
            table.entry(key).or_default().extend(records);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.table().borrow().is_empty()
    }
}

/// Hit scopes of the slots met while prepainting the copies, by (segment, root).
#[derive(Clone, Default)]
pub(crate) struct SlotScopes(Rc<RefCell<Vec<(PaintSegmentId, u64, PaintHitScope)>>>);

impl SlotScopes {
    pub(crate) fn push(&self, segment: PaintSegmentId, root: u64, scope: PaintHitScope) {
        let cell: &RefCell<Vec<(PaintSegmentId, u64, PaintHitScope)>> = &self.0;
        cell.borrow_mut().push((segment, root, scope));
    }

    /// Forget the slots of the previous prepaint.
    pub(crate) fn clear(&self) {
        let cell: &RefCell<Vec<(PaintSegmentId, u64, PaintHitScope)>> = &self.0;
        cell.borrow_mut().clear();
    }

    /// Scopes of the placements `(segment, ordinal, root)` that a slot replays.
    pub(crate) fn scopes(
        &self,
        placements: &[(PaintSegmentId, u32, u64)],
    ) -> Rc<[(PaintSegmentId, u32, PaintHitScope)]> {
        let cell: &RefCell<Vec<(PaintSegmentId, u64, PaintHitScope)>> = &self.0;
        let slots = cell.borrow();
        placements
            .iter()
            .filter_map(|&(segment, ordinal, root)| {
                let (_, _, scope) = slots
                    .iter()
                    .rev()
                    .find(|(s, r, _)| *s == segment && *r == root)?;
                Some((segment, ordinal, scope.clone()))
            })
            .collect()
    }
}
