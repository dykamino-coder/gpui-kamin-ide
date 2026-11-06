//! KaminIDE patch: reusing a frame's line layouts whole equals reusing them
//! one by one, and a range past a frame reused whole reuses nothing.

use super::*;
use crate::NoopTextSystem;

fn cache() -> LineLayoutCache {
    LineLayoutCache::new(Arc::new(NoopTextSystem), Arc::new(AtomicUsize::new(0)))
}

fn line(cache: &LineLayoutCache, text: &'static str) -> Arc<LineLayout> {
    let run = FontRun {
        break_ligatures: true,
        len: text.len(),
        font_id: FontId(0),
        font_size: px(0.),
    };
    cache.layout_line(text, px(16.), &[run], None)
}

fn used(frame: &FrameCache) -> Vec<String> {
    frame
        .used_lines
        .iter()
        .map(|key| key.text.to_string())
        .collect()
}

fn cached(frame: &FrameCache) -> Vec<String> {
    let mut texts: Vec<String> = frame.lines.keys().map(|key| key.text.to_string()).collect();
    texts.sort();
    texts
}

/// Lays out `texts` in a frame and finishes it; returns the frame's range.
fn frame(cache: &LineLayoutCache, texts: &[&'static str]) -> Range<LineLayoutIndex> {
    let start = cache.layout_index();
    for text in texts {
        line(cache, text);
    }
    let end = cache.layout_index();
    cache.finish_frame();
    start..end
}

#[test]
fn reusing_a_whole_frame_keeps_every_layout_in_its_order() {
    let cache = cache();
    let start = cache.layout_index();
    let first = line(&cache, "a");
    line(&cache, "b");
    let range = start..cache.layout_index();
    cache.finish_frame();
    cache.reuse_layouts(range.clone());
    {
        let current = cache.current_frame.read();
        assert_eq!(used(&current), ["a", "b"]);
        assert_eq!(cached(&current), ["a", "b"]);
        assert!(cache.previous_frame.lock().lines.is_empty());
    }
    // The paint range after it is past the previous frame's lists now.
    cache.reuse_layouts(range.end.clone()..range.end.clone());
    // Reused layouts are the same allocations, found without shaping again.
    assert!(Arc::ptr_eq(&line(&cache, "a"), &first));
    assert_eq!(used(&cache.current_frame.read()), ["a", "b"]);
    // The next frame replays the same range again.
    cache.finish_frame();
    cache.reuse_layouts(range);
    assert_eq!(used(&cache.current_frame.read()), ["a", "b"]);
}

#[test]
fn reusing_part_of_a_frame_moves_only_that_part() {
    let cache = cache();
    let start = cache.layout_index();
    line(&cache, "a");
    let middle = cache.layout_index();
    line(&cache, "b");
    let end = cache.layout_index();
    cache.finish_frame();
    cache.reuse_layouts(middle.clone()..end.clone());
    assert_eq!(used(&cache.current_frame.read()), ["b"]);
    assert_eq!(cached(&cache.previous_frame.lock()), ["a"]);
    cache.reuse_layouts(start..middle);
    assert_eq!(used(&cache.current_frame.read()), ["b", "a"]);
    assert_eq!(cached(&cache.current_frame.read()), ["a", "b"]);
}

#[test]
fn a_frame_that_laid_out_lines_already_reuses_one_by_one() {
    let cache = cache();
    let range = frame(&cache, &["a", "b"]);
    line(&cache, "c");
    cache.reuse_layouts(range);
    assert_eq!(used(&cache.current_frame.read()), ["c", "a", "b"]);
    assert_eq!(cached(&cache.current_frame.read()), ["a", "b", "c"]);
}

#[test]
fn a_frame_with_a_layout_cached_twice_is_not_swapped() {
    let cache = cache();
    let start = cache.layout_index();
    line(&cache, "a");
    line(&cache, "b");
    // A truncated tail leaves "b" cached without a use.
    let mut index = cache.layout_index();
    index.lines_index = 1;
    cache.truncate_layouts(index.clone());
    cache.finish_frame();
    cache.reuse_layouts(start..index);
    assert_eq!(used(&cache.current_frame.read()), ["a"]);
    assert_eq!(cached(&cache.current_frame.read()), ["a"]);
    assert_eq!(cached(&cache.previous_frame.lock()), ["b"]);
}
