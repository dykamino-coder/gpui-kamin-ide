//! Мера рядов колонок одного `SpannerFlow`, общая для его измерительной и
//! финальной сборки.

use gpui::Pixels;
use std::cell::RefCell;
use std::rc::Rc;

/// Высота ряда и его первая/последняя базовые линии (`Recipe::measure`).
pub(crate) type RowExtent = (f32, Option<Pixels>, Option<Pixels>);

/// Resolved space of one `SpannerFlow` build: width and definite height bits.
pub(crate) type BuildSpace = (u32, Option<u32>);

/// Row extents measured by the isolated measure build of a spanner flow.
///
/// Taffy sizes the flow with a `SpannerPass::Measure` build, and the final
/// build at the same resolved space lays every row out again only to answer
/// the same size query before its prepaint lays the row out for paint. The
/// measure build has no fragmentation recorder and fresh positioning and
/// paint collectors, so a row's extent there is a function of the build
/// space, the row and the column-set width; the final build has the same
/// inputs when it records no fragmentation either (`FragCx::watch`), since
/// out-of-flow boxes and paint collectors do not size the row. Blink keeps
/// one layout result per box and constraint space (`layout_result_caching.cc`)
/// and reuses it for the same reason.
#[derive(Default)]
pub(crate) struct RowMeasures {
    space: std::cell::Cell<Option<BuildSpace>>,
    rows: RefCell<Vec<((usize, u32), RowExtent)>>,
}

/// The slot of one spanner flow's rows in `FragmentationContext`.
#[derive(Clone)]
pub(crate) struct RowSlot {
    memo: Rc<RowMeasures>,
    /// The multicol element whose rows these are.
    host: u64,
    space: BuildSpace,
    /// Row index, set for the row being built (`multicol_spanner_segments`).
    row: Option<usize>,
    /// Only the measure build records; the final one reads.
    record: bool,
}

impl RowSlot {
    /// The slot of a spanner flow build, `None` when it records fragmentation
    /// or has no element identity.
    pub(crate) fn build(
        memo: &Rc<RowMeasures>,
        host: u64,
        space: BuildSpace,
        record: bool,
        fragmented: bool,
    ) -> Option<Self> {
        if fragmented || host == 0 {
            return None;
        }
        if record && memo.space.get() != Some(space) {
            memo.space.set(Some(space));
            memo.rows.borrow_mut().clear();
        }
        Some(Self {
            memo: memo.clone(),
            host,
            space,
            row: None,
            record,
        })
    }

    /// The same slot for row `row` of `host`, `None` for another element.
    pub(crate) fn row(&self, host: u64, row: usize) -> Option<Self> {
        (self.host == host && self.row.is_none()).then(|| Self {
            row: Some(row),
            ..self.clone()
        })
    }

    /// Whether this is the slot of a row of `host`.
    pub(crate) fn is_row_of(&self, host: u64) -> bool {
        self.host == host && self.row.is_some()
    }

    pub(crate) fn get(&self, width: f32) -> Option<RowExtent> {
        let row = self.row?;
        if self.memo.space.get() != Some(self.space) {
            return None;
        }
        let key = (row, width.to_bits());
        let rows = self.memo.rows.borrow();
        rows.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, extent)| *extent)
    }

    pub(crate) fn put(&self, width: f32, extent: RowExtent) {
        let Some(row) = self.row.filter(|_| self.record) else {
            return;
        };
        if self.memo.space.get() != Some(self.space) {
            return;
        }
        let key = (row, width.to_bits());
        let mut rows = self.memo.rows.borrow_mut();
        match rows.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = extent,
            None => rows.push((key, extent)),
        }
    }
}
