//! Записи временного layout-движка и геометрия, переживающая его освобождение.

use super::ClampEntry;
use crate::layout::probe::{Measure, MeasureScope as LayoutScope};
use gpui::{AnyElement, Bounds, LayoutId, Pixels, Point, Window};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

type Entry = Rc<dyn Fn(Bounds<Pixels>) -> ClampEntry>;
type Rows = HashMap<(u64, u32), Vec<(f32, f32)>>;

type Lifted = Rc<RefCell<MeasuredClamp>>;

#[derive(Clone, Default)]
pub(crate) struct ClampMeasure {
    entries: Measure<Entry>,
    lifted: Measure<Lifted>,
    rows: Rc<RefCell<Rows>>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MeasuredClamp {
    pub(crate) entries: Vec<ClampEntry>,
    pub(crate) rows: HashMap<u32, Vec<(f32, f32)>>,
}

impl MeasuredClamp {
    pub(crate) fn append(&mut self, other: &Self, origin: Point<Pixels>) {
        self.entries
            .extend(other.entries.iter().cloned().map(|mut e| {
                e.bounds.origin += origin;
                e
            }));
        for (seq, rows) in &other.rows {
            self.rows.insert(
                *seq,
                rows.iter()
                    .map(|(a, b)| {
                        let y = f32::from(origin.y);
                        (a + y, b + y)
                    })
                    .collect(),
            );
        }
    }
}

impl ClampMeasure {
    pub(crate) fn publish_rows(&self, tag: (u64, u32), rows: Vec<(f32, f32)>) {
        self.rows.borrow_mut().insert(tag, rows);
    }

    fn take_rows(&self, key: u64) -> HashMap<u32, Vec<(f32, f32)>> {
        let mut rows = self.rows.borrow_mut();
        let tags: Vec<_> = rows.keys().filter(|tag| tag.0 == key).copied().collect();
        tags.into_iter()
            .filter_map(|tag| {
                rows.remove(&tag).map(|mut row| {
                    row.sort_by(|a, b| a.0.total_cmp(&b.0));
                    (tag.1, row)
                })
            })
            .collect()
    }

    pub(crate) fn record(&self, f: impl Fn(Bounds<Pixels>) -> ClampEntry + 'static) -> AnyElement {
        self.entries.probe(Rc::new(f) as Entry)
    }

    pub(crate) fn lift(&self, id: LayoutId, batch: Lifted) {
        self.lifted.record(id, batch);
    }

    pub(crate) fn collect(
        &self,
        key: u64,
        origin: Point<Pixels>,
        window: &mut Window,
    ) -> MeasuredClamp {
        let mut rows = self.take_rows(key);
        let mut batch = MeasuredClamp::default();
        for (bounds, entry) in self.entries.take(window) {
            let at = bounds.origin - origin;
            let e = entry(Bounds {
                origin: at,
                size: bounds.size,
            });
            if let Some(seq) = e.seq
                && let Some(r) = rows.remove(&seq)
            {
                let y = f32::from(at.y);
                batch
                    .rows
                    .insert(seq, r.into_iter().map(|(a, b)| (a + y, b + y)).collect());
            }
            batch.entries.push(e);
        }
        for (bounds, lifted) in self.lifted.take(window) {
            batch.append(&lifted.borrow(), bounds.origin - origin);
        }
        batch
    }

    pub(crate) fn enter(&self, key: u64) -> MeasureScope {
        MeasureScope {
            measure: self.clone(),
            key,
            _entries: self.entries.enter(),
            _lifted: self.lifted.enter(),
            rows: self.take_rows(key),
        }
    }
}

/// Вложенная раскладка не оставляет LayoutId в родительском движке.
pub(crate) struct MeasureScope {
    measure: ClampMeasure,
    key: u64,
    _entries: LayoutScope<Entry>,
    _lifted: LayoutScope<Lifted>,
    rows: HashMap<u32, Vec<(f32, f32)>>,
}

impl MeasureScope {
    pub(crate) fn finish(self, root: LayoutId, window: &mut Window) -> MeasuredClamp {
        self.measure
            .collect(self.key, window.layout_origin_unrounded(root), window)
    }
}

impl Drop for MeasureScope {
    fn drop(&mut self) {
        self.measure.take_rows(self.key);
        for (seq, rows) in self.rows.drain() {
            self.measure.publish_rows((self.key, seq), rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_measurements_restore_only_the_enclosing_rows() {
        let measure = ClampMeasure::default();
        measure.publish_rows((7, 0), vec![(10., 20.)]);
        measure.publish_rows((8, 0), vec![(30., 40.)]);
        {
            let _outer = measure.enter(7);
            assert!(measure.take_rows(7).is_empty());
            measure.publish_rows((7, 1), vec![(50., 60.)]);
            {
                let _inner = measure.enter(7);
                measure.publish_rows((7, 2), vec![(70., 80.)]);
            }
            let rows = measure.take_rows(7);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[&1], vec![(50., 60.)]);
        }
        let rows = measure.take_rows(7);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[&0], vec![(10., 20.)]);
        assert_eq!(measure.take_rows(8)[&0], vec![(30., 40.)]);
    }

    #[test]
    fn measurements_with_identical_tags_have_independent_row_buffers() {
        let a = ClampMeasure::default();
        let b = ClampMeasure::default();
        a.publish_rows((7, 0), vec![(20., 25.), (10., 15.)]);
        b.publish_rows((7, 0), vec![(30., 40.)]);
        assert_eq!(a.take_rows(7)[&0], vec![(10., 15.), (20., 25.)]);
        assert!(a.take_rows(7).is_empty());
        assert_eq!(b.take_rows(7)[&0], vec![(30., 40.)]);
        assert!(ClampMeasure::default().take_rows(7).is_empty());
    }

    #[test]
    fn lifted_rows_follow_the_physical_slot_origin() {
        let measured = MeasuredClamp {
            rows: HashMap::from([(2, vec![(0., 12.), (12., 32.)])]),
            ..Default::default()
        };
        let mut lifted = MeasuredClamp::default();
        lifted.append(&measured, gpui::point(gpui::px(5.5), gpui::px(20.25)));
        assert_eq!(lifted.rows[&2], vec![(20.25, 32.25), (32.25, 52.25)]);
        assert_eq!(measured.rows[&2], vec![(0., 12.), (12., 32.)]);
    }
}
