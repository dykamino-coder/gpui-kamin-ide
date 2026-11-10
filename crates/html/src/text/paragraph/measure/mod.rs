//! Измерения абзацев, принадлежащие документу, и результаты text-fit от измерения к отрисовке.

mod bidi_levels;
mod cache_keys;
mod chunked;
mod chunks;
mod flow_cut;
mod frame_cache;
mod geometry_key;
mod min_content;
mod padding;
mod width;
#[cfg(test)]
mod width_tests;

use crate::text::paragraph::{Line, Seg};
use frame_cache::FrameCache;
use std::{cell::RefCell, rc::Rc};

/// Paragraphs and their layout probes share only their document's measurements.
#[derive(Default)]
pub(crate) struct MeasureCache {
    pub(in crate::text::paragraph) splits: RefCell<FrameCache<Rc<Vec<Line>>>>,
    pub(in crate::text::paragraph) fitted: RefCell<FrameCache<f32>>,
    pub(in crate::text::paragraph) measured: RefCell<FrameCache<Vec<Seg>>>,
    /// Наборы частей длинных кусков и списки частей их абзацев (`chunks`).
    pub(in crate::text::paragraph) shaped: RefCell<FrameCache<Shaped>>,
    /// Line splits computed because the cache had no entry (observation for tests).
    #[cfg(test)]
    pub(crate) split_misses: std::cell::Cell<usize>,
}

impl MeasureCache {
    pub(crate) fn begin_frame(&self) {
        self.splits.borrow_mut().begin_frame();
        self.measured.borrow_mut().begin_frame();
        self.shaped.borrow_mut().begin_frame_keeping();
        self.fitted.borrow_mut().begin_frame();
    }

    /// Запись памяти частей по ключу; чтение продлевает её жизнь на кадр.
    fn chunk(&self, key: u64) -> Option<Shaped> {
        self.shaped.borrow_mut().get(&key).cloned()
    }

    fn put_chunk(&self, key: u64, value: Shaped) {
        self.shaped.borrow_mut().insert(key, value);
    }

    pub(super) fn remember_fit(&self, key: u64, k: f32) {
        self.fitted.borrow_mut().insert(key, k);
    }
}

/// Запись памяти частей: набор части или список частей одного замера.
#[derive(Clone)]
pub(in crate::text::paragraph) enum Shaped {
    Chunk(chunked::Chunk),
    Keys(Rc<[u64]>),
}

impl crate::text::paragraph::Paragraph {
    /// Запомнить части абзаца с ключом замера `key` (абзац без частей — ничего).
    fn remember_chunks(&self, key: u64, chunks: &chunked::Keys) {
        if chunks.len > 0 {
            let list = Shaped::Keys(chunks.keys[..chunks.len].into());
            self.measures.put_chunk(list_key(key), list);
        }
    }

    /// Части держатся в памяти, пока держится замер их абзаца.
    fn keep_chunks(&self, key: u64) {
        if let Some(Shaped::Keys(keys)) = self.measures.chunk(list_key(key)) {
            keys.iter().for_each(|&k| _ = self.measures.chunk(k));
        }
    }
}

/// Ключ списка частей замера `key`: не совпадает с ключами самих частей.
fn list_key(key: u64) -> u64 {
    key.rotate_left(29) ^ 0x9E37_79B9_7F4A_7C15
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
