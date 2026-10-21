//! Измерения абзацев, принадлежащие документу, и результаты text-fit от измерения к отрисовке.

mod cache_keys;
mod flow_cut;
mod min_content;
mod padding;
mod width;

use crate::text::paragraph::{Line, Seg};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// Paragraphs and their layout probes share only their document's measurements.
#[derive(Default)]
pub(crate) struct MeasureCache {
    pub(crate) splits: RefCell<HashMap<u64, Rc<Vec<Line>>>>,
    pub(crate) fitted: RefCell<Vec<(u64, f32)>>,
    pub(crate) measured: RefCell<Vec<(u64, Vec<Seg>)>>,
}

/// Number of measurements and fitted multipliers retained between frames.
const MEASURE_CACHE: usize = 64;

impl MeasureCache {
    pub(super) fn remember_fit(&self, key: u64, k: f32) {
        let mut cache = self.fitted.borrow_mut();
        if let Some(hit) = cache.iter_mut().find(|(hit, _)| *hit == key) {
            hit.1 = k;
            return;
        }
        if cache.len() >= MEASURE_CACHE {
            cache.remove(0);
        }
        cache.push((key, k));
    }
}
