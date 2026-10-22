//! Счётчики приближённой меры: только наблюдение, без повторной раскладки.

use crate::tolerance::COORD_EPS;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Накопленные наблюдения; ключ — место сравнения оценки с watched.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FragmentProbe {
    pub estimate_calls: u64,
    /// Реальные входы оценки; суммарное число вызовов остаётся estimate_calls.
    pub estimate_sites: BTreeMap<String, u64>,
    pub height_comparisons: u64,
    pub height_mismatches: u64,
    pub underestimated: u64,
    pub max_height_error: f32,
    pub total_height_error: f64,
    pub break_comparisons: u64,
    pub break_mismatches: u64,
}

/// Явный сборщик прогона в FragCx; клоны разделяют только этот сборщик.
#[derive(Clone, Debug, Default)]
pub struct FragmentProbes(Rc<RefCell<BTreeMap<String, FragmentProbe>>>);

impl FragmentProbes {
    /// Забрать и обнулить наблюдения этого контекста.
    pub fn take(&self) -> BTreeMap<String, FragmentProbe> {
        std::mem::take(&mut *self.0.borrow_mut())
    }

    #[track_caller]
    pub(crate) fn estimate(&self, site: &str) {
        let caller = std::panic::Location::caller();
        let mut counters = self.0.borrow_mut();
        let probe = counters.entry(site.into()).or_default();
        probe.estimate_calls += 1;
        *probe
            .estimate_sites
            .entry(format!("{}:{}", caller.file(), caller.line()))
            .or_default() += 1;
    }

    pub(crate) fn height(&self, site: &str, estimate: f32, laid: f32) {
        self.0
            .borrow_mut()
            .entry(site.into())
            .or_default()
            .height(estimate, laid);
    }

    pub(crate) fn breaks(&self, site: &str, old: &[(f32, f32)], new: &[(f32, f32)]) {
        let same = old.len() == new.len()
            && old
                .iter()
                .zip(new)
                .all(|(a, b)| (a.0 - b.0).abs() <= COORD_EPS && (a.1 - b.1).abs() <= COORD_EPS);
        let mut counters = self.0.borrow_mut();
        let p = counters.entry(site.into()).or_default();
        p.break_comparisons += 1;
        p.break_mismatches += u64::from(!same);
    }
}

impl FragmentProbe {
    fn height(&mut self, estimate: f32, laid: f32) {
        self.height_comparisons += 1;
        let error = (estimate - laid).abs();
        if error > COORD_EPS || !error.is_finite() {
            self.height_mismatches += 1;
            self.underestimated += u64::from(estimate < laid);
            self.max_height_error = self.max_height_error.max(error);
            self.total_height_error += f64::from(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_error_without_changing_geometry() {
        let mut p = FragmentProbe::default();
        p.height(100.0, 100.0 + COORD_EPS / 2.0);
        p.height(80.0, 100.0);
        p.height(110.0, 100.0);
        assert_eq!(p.height_comparisons, 3);
        assert_eq!(p.height_mismatches, 2);
        assert_eq!(p.underestimated, 1);
        assert_eq!(p.max_height_error, 20.0);
        assert_eq!(p.total_height_error, 30.0);
    }

    #[test]
    fn snapshot_resets_only_its_explicit_context() {
        let a = FragmentProbes::default();
        let b = FragmentProbes::default();
        a.estimate("test");
        a.estimate("test");
        a.breaks("test", &[(1.0, 2.0)], &[(1.0, 3.0)]);
        assert!(b.take().is_empty());
        let clone = a.clone();
        let p = clone.take();
        assert_eq!(p["test"].estimate_calls, 2);
        assert_eq!(p["test"].estimate_sites.len(), 2);
        assert_eq!(p["test"].estimate_sites.values().sum::<u64>(), 2);
        assert_eq!(p["test"].break_comparisons, 1);
        assert_eq!(p["test"].break_mismatches, 1);
        assert!(a.take().is_empty());
    }
}
