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

#[cfg(test)]
mod tests {
    #[test]
    fn document_and_render_advance_measurements_once_per_frame() {
        let doc = crate::Document::builder("<p>text</p>", "").build().unwrap();
        let opts = crate::RenderOpts {
            text: Default::default(),
            viewport: (800., 600.),
            normal_line_height: 1.31,
            cx: Default::default(),
            frag: Default::default(),
        };
        let first = doc.frame_opts(&opts);
        let cache = first.cx.paragraph().paragraphs.clone();
        crate::interactive::frame::frame_sanitize(&first.cx);
        cache.fitted.borrow_mut().insert(1, 16.);
        for _ in 0..3 {
            let next = doc.frame_opts(&opts);
            crate::interactive::frame::frame_sanitize(&next.cx);
            // Повторный render в одном кадре документа не меняет поколение.
            crate::interactive::frame::frame_sanitize(&next.cx);
            assert_eq!(cache.fitted.borrow_mut().get(&1), Some(&16.));
        }
        doc.frame_opts(&opts);
        doc.frame_opts(&opts);
        assert!(cache.fitted.borrow_mut().get(&1).is_none());
    }

    #[test]
    fn low_level_render_without_document_still_ages_its_measurements() {
        let cx = crate::layout::cx::LayoutCx::default();
        let cache = &cx.paragraph().paragraphs;
        cache.fitted.borrow_mut().insert(1, 16.);
        crate::interactive::frame::frame_sanitize(&cx);
        assert_eq!(cache.fitted.borrow_mut().get(&1), Some(&16.));
        crate::interactive::frame::frame_sanitize(&cx);
        crate::interactive::frame::frame_sanitize(&cx);
        assert!(cache.fitted.borrow_mut().get(&1).is_none());
    }
}
