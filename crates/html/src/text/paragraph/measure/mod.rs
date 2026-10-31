//! Измерения абзацев, принадлежащие документу, и результаты text-fit от измерения к отрисовке.

mod cache_keys;
mod flow_cut;
mod frame_cache;
mod geometry_key;
mod min_content;
mod padding;
mod width;

use crate::text::paragraph::{Line, Seg};
use frame_cache::FrameCache;
use std::{cell::RefCell, rc::Rc};

/// Paragraphs and their layout probes share only their document's measurements.
#[derive(Default)]
pub(crate) struct MeasureCache {
    pub(in crate::text::paragraph) splits: RefCell<FrameCache<Rc<Vec<Line>>>>,
    pub(in crate::text::paragraph) fitted: RefCell<FrameCache<f32>>,
    pub(in crate::text::paragraph) measured: RefCell<FrameCache<Vec<Seg>>>,
}

impl MeasureCache {
    pub(crate) fn begin_frame(&self) {
        self.splits.borrow_mut().begin_frame();
        self.measured.borrow_mut().begin_frame();
        self.fitted.borrow_mut().begin_frame();
    }

    pub(super) fn remember_fit(&self, key: u64, k: f32) {
        self.fitted.borrow_mut().insert(key, k);
    }
}
