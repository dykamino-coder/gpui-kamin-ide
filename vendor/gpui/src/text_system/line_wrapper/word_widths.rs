//! KaminIDE patch: widths of unbreakable text pieces for `min_content_width`.
//!
//! The min-content width of a text is its widest piece between break
//! opportunities, and each piece is measured by the platform shaper
//! (`layout_line`), as wrapping later places it. A piece's shaping depends
//! only on the font, the size and its characters (one run, ligatures break),
//! so the same word under the same font has the same width. Without a memory
//! every min-content query shaped every word again (DirectWrite builds a text
//! layout per word), a tenth of a long article's cold load.
//!
//! The key is the piece's text; font and size belong to the owner (the
//! `LineWrapper` of a pool per font and size). A word's width does not depend
//! on layout, so the memory outlives a frame: a text frame takes the widths
//! the previous frame read (as `LineLayoutCache` keeps the previous frame's
//! lines), and an edit or a new window width shapes no word again. A word no
//! frame read leaves in the next one: the memory is the last frame's working
//! set, with no timer and no growth over time; a frame the wrapper skipped
//! clears it. A frame that reads no width costs nothing here: the text frame
//! is a counter, and a wrapper turns its generations over only at its next
//! read (`WordWidths::width`). Blink keeps shaped words in its
//! `FrameShapeCache` (evicted by frames, per font), Gecko in the word cache of
//! `gfxFont`, evicted by the `gfxFontCache` timer; here eviction is
//! deterministic by frames, and the lookup key is the text slice itself, with
//! no allocation (only a new word allocates). The font generation resets the
//! memory: an added font may change glyph fallback. Within a frame the memory
//! is bounded by two generations of `GENERATION_WORDS` entries; frequent
//! words move into the current one. Pieces longer than `LONGEST_WORD` bytes
//! (addresses, long numbers) are measured without memory: they rarely repeat
//! and are costly to keep.

use crate::Pixels;
use collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Entries in one generation. A text's frequent words (function words,
/// repeated terms) fit in it; rare ones leave when the generation turns over.
const GENERATION_WORDS: usize = 8192;

/// The longest piece kept, in bytes.
const LONGEST_WORD: usize = 64;

/// The text frame number: `LineLayoutCache::finish_frame` of any window advances it.
static TEXT_FRAME: AtomicUsize = AtomicUsize::new(0);

/// End of a text frame: the width memory of every wrapper turns over at its
/// next read. One counter step, whatever the wrappers hold.
pub(crate) fn finish_frame() {
    TEXT_FRAME.fetch_add(1, Ordering::Relaxed);
}

/// The current text frame.
pub(crate) fn text_frame() -> usize {
    TEXT_FRAME.load(Ordering::Relaxed)
}

#[derive(Default)]
pub(crate) struct WordWidths {
    /// (font generation, text frame) the widths were recorded under.
    stamp: (usize, usize),
    current: HashMap<Box<str>, Pixels>,
    previous: HashMap<Box<str>, Pixels>,
}

impl WordWidths {
    /// The width of piece `word` in frame `frame`, from memory or from
    /// `measure` (the platform shaper).
    pub(crate) fn width(
        &mut self,
        font_generation: usize,
        frame: usize,
        word: &str,
        measure: impl FnOnce() -> Pixels,
    ) -> Pixels {
        if word.len() > LONGEST_WORD {
            return measure();
        }
        let stamp = (font_generation, frame);
        if self.stamp != stamp {
            let next = font_generation == self.stamp.0 && frame == self.stamp.1.wrapping_add(1);
            // What the last frame read stays; anything older leaves. New
            // tables rather than `clear`: an old generation keeps no capacity.
            self.previous = if next {
                std::mem::take(&mut self.current)
            } else {
                self.current = HashMap::default();
                HashMap::default()
            };
            self.stamp = stamp;
        }
        if let Some(width) = self.current.get(word) {
            return *width;
        }
        let (key, width) = match self.previous.remove_entry(word) {
            Some(entry) => entry,
            None => (Box::from(word), measure()),
        };
        if self.current.len() >= GENERATION_WORDS {
            self.previous = std::mem::take(&mut self.current);
        }
        self.current.insert(key, width);
        width
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.current.len() + self.previous.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;

    #[test]
    fn a_repeated_word_is_measured_once_per_frame_and_font_generation() {
        let mut widths = WordWidths::default();
        let mut calls = 0;
        let mut width = |widths: &mut WordWidths, generation, frame| {
            widths.width(generation, frame, "word", || {
                calls += 1;
                px(12.5 + generation as f32)
            })
        };
        for _ in 0..3 {
            assert_eq!(width(&mut widths, 0, 0), px(12.5));
        }
        // A new font in the system: the old widths do not hold.
        assert_eq!(width(&mut widths, 1, 0), px(13.5));
        // The next frame takes the width the last one read.
        assert_eq!(width(&mut widths, 1, 1), px(13.5));
        assert_eq!(width(&mut widths, 1, 2), px(13.5));
        assert_eq!(widths.len(), 1);
        // A frame the wrapper skipped clears its memory.
        assert_eq!(width(&mut widths, 1, 4), px(13.5));
        drop(width);
        assert_eq!(calls, 3);
    }

    #[test]
    fn a_word_unread_by_a_frame_leaves_in_the_next() {
        let mut widths = WordWidths::default();
        let mut measured = Vec::new();
        let mut read = |widths: &mut WordWidths, frame, word: &'static str| {
            widths.width(0, frame, word, || {
                measured.push(word);
                px(1.)
            });
        };
        read(&mut widths, 0, "kept");
        read(&mut widths, 0, "dropped");
        read(&mut widths, 1, "kept");
        assert_eq!(widths.len(), 2, "the last frame's words are still there");
        read(&mut widths, 2, "kept");
        assert_eq!(widths.len(), 1);
        read(&mut widths, 2, "dropped");
        let long = "x".repeat(LONGEST_WORD + 1);
        widths.width(0, 2, &long, || px(3.));
        widths.width(0, 2, &long, || px(3.));
        assert_eq!(widths.len(), 2, "a long piece is not kept");
        drop(read);
        assert_eq!(measured, ["kept", "dropped", "dropped"]);
    }

    #[test]
    fn frames_that_read_no_width_leave_the_memory_untouched() {
        let mut widths = WordWidths::default();
        for i in 0..100 {
            widths.width(0, 0, &format!("w{i}"), || px(1.));
        }
        // Frames without layout only advance the counter: the tables stay
        // as they are until a read, and the turn-over then drops them.
        let before = text_frame();
        for _ in 0..1000 {
            finish_frame();
        }
        assert!(text_frame() >= before + 1000);
        assert_eq!(widths.len(), 100);
        widths.width(0, 1000, "w0", || px(1.));
        assert_eq!(widths.len(), 1);
    }

    #[test]
    fn memory_is_bounded_by_two_generations_and_keeps_frequent_words() {
        let mut widths = WordWidths::default();
        let mut measured = 0;
        for i in 0..(GENERATION_WORDS * 5) {
            widths.width(0, 0, &format!("w{i}"), || {
                measured += 1;
                px(1.)
            });
            // A frequent word between rare ones is not shaped again.
            widths.width(0, 0, "the", || {
                measured += 1;
                px(2.)
            });
            assert!(widths.len() <= 2 * GENERATION_WORDS);
        }
        assert_eq!(measured, GENERATION_WORDS * 5 + 1);
    }
}
