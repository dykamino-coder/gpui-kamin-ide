//! Записи временного layout-движка и геометрия, переживающая его освобождение.

use super::{ClampEntry, ParagraphRows};
use crate::layout::probe::{Sink, layout_record};
use gpui::{AnyElement, Bounds, LayoutId, Pixels, Point, Window};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

type Entry = Rc<dyn Fn(Bounds<Pixels>) -> ClampEntry>;
type Lifted = Rc<RefCell<MeasuredClamp>>;

#[derive(Clone, Default)]
pub(crate) struct ClampMeasure {
    entries: Sink<(LayoutId, Entry)>,
    lifted: Sink<(LayoutId, Lifted)>,
    pub(crate) rows: Rc<ParagraphRows>,
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
    pub(crate) fn record(&self, f: impl Fn(Bounds<Pixels>) -> ClampEntry + 'static) -> AnyElement {
        layout_record(self.entries.clone(), Rc::new(f) as Entry)
    }

    pub(crate) fn lift(&self, id: LayoutId, batch: Lifted) {
        self.lifted.push((id, batch));
    }

    pub(crate) fn collect(
        &self,
        key: u64,
        origin: Point<Pixels>,
        window: &mut Window,
    ) -> MeasuredClamp {
        let mut rows = self.rows.take(key);
        let mut batch = MeasuredClamp::default();
        for (id, entry) in self.entries.take() {
            let at = window.layout_origin_unrounded(id) - origin;
            let e = entry(Bounds {
                origin: at,
                size: window.layout_size_unrounded(id),
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
        for (id, lifted) in self.lifted.take() {
            batch.append(
                &lifted.borrow(),
                window.layout_origin_unrounded(id) - origin,
            );
        }
        batch
    }

    pub(crate) fn enter(&self, key: u64) -> MeasureScope {
        MeasureScope {
            measure: self.clone(),
            key,
            entries: self.entries.take(),
            lifted: self.lifted.take(),
            rows: self.rows.take(key),
        }
    }
}

/// Вложенная раскладка не оставляет LayoutId в родительском движке.
pub(crate) struct MeasureScope {
    measure: ClampMeasure,
    key: u64,
    entries: Vec<(LayoutId, Entry)>,
    lifted: Vec<(LayoutId, Lifted)>,
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
        self.measure.entries.take();
        self.measure.lifted.take();
        self.measure.rows.take(self.key);
        for e in self.entries.drain(..) {
            self.measure.entries.push(e);
        }
        for e in self.lifted.drain(..) {
            self.measure.lifted.push(e);
        }
        for (seq, rows) in self.rows.drain() {
            self.measure.rows.publish((self.key, seq), rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_measurements_restore_only_the_enclosing_rows() {
        let measure = ClampMeasure::default();
        measure.rows.publish((7, 0), vec![(10., 20.)]);
        measure.rows.publish((8, 0), vec![(30., 40.)]);
        {
            let _outer = measure.enter(7);
            assert!(measure.rows.take(7).is_empty());
            measure.rows.publish((7, 1), vec![(50., 60.)]);
            {
                let _inner = measure.enter(7);
                measure.rows.publish((7, 2), vec![(70., 80.)]);
            }
            let rows = measure.rows.take(7);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[&1], vec![(50., 60.)]);
        }
        let rows = measure.rows.take(7);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[&0], vec![(10., 20.)]);
        assert_eq!(measure.rows.take(8)[&0], vec![(30., 40.)]);
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
