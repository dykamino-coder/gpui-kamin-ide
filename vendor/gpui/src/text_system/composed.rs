//! KaminIDE patch: a line laid out from pieces shaped apart. A caller that
//! cuts a long line where the shaper breaks it anyway (no kerning, ligature or
//! contextual form across the cut) composes the line from its pieces: the line
//! is cached like any other, under the key of its whole text, and its pieces
//! are kept beside it by the caller's keys, so an edit of the text reshapes
//! only the pieces it touches and copies the glyphs of the others.
//!
//! Pieces age by the frames that use them: a frame that neither looks up nor
//! stores a piece leaves them all in place (an idle window keeps them for the
//! next edit), and a piece unused for one whole active frame is dropped.

use super::{LineLayout, TextRun, WindowTextSystem};
use crate::{Pixels, SharedString};
use collections::FxHashMap;
use parking_lot::Mutex;
use std::{ops::Range, sync::Arc};

/// A piece of a composed line: its glyph runs in the line `layout`.
#[derive(Clone)]
pub struct LinePiece {
    /// The composed line the piece was laid out in.
    pub layout: Arc<LineLayout>,
    /// The glyph runs of the piece in `layout`.
    pub runs: Range<u32>,
    /// Byte offset of the piece in the text of `layout`.
    pub base: u32,
    /// Pen position where the piece starts in `layout`.
    pub origin: Pixels,
    /// Advance of the piece (letter-spacing included).
    pub width: Pixels,
    /// Ascent of the piece's runs.
    pub ascent: Pixels,
    /// Descent of the piece's runs.
    pub descent: Pixels,
}

#[derive(Default)]
struct Frame {
    pieces: FxHashMap<u64, LinePiece>,
    lists: FxHashMap<u64, Arc<[u64]>>,
}

#[derive(Default)]
struct Frames {
    current: Frame,
    previous: Frame,
    used: bool,
}

/// Pieces of composed lines, kept for the current and the previous active frame.
#[derive(Default)]
pub(crate) struct PieceCache(Mutex<Frames>);

impl PieceCache {
    fn get(&self, key: u64) -> Option<LinePiece> {
        let mut f = self.0.lock();
        f.used = true;
        if let Some(piece) = f.current.pieces.get(&key) {
            return Some(piece.clone());
        }
        let piece = f.previous.pieces.remove(&key)?;
        f.current.pieces.insert(key, piece.clone());
        Some(piece)
    }

    fn put(&self, key: u64, piece: LinePiece) {
        let mut f = self.0.lock();
        f.used = true;
        f.current.pieces.insert(key, piece);
    }

    /// Keep the list `list` and its pieces for this frame; `false` when it is gone.
    fn keep(&self, list: u64) -> bool {
        let f = &mut *self.0.lock();
        f.used = true;
        let keys = match f.current.lists.get(&list) {
            Some(keys) => keys.clone(),
            None => match f.previous.lists.remove(&list) {
                Some(keys) => {
                    f.current.lists.insert(list, keys.clone());
                    keys
                }
                None => return false,
            },
        };
        for key in keys.iter() {
            if let Some(piece) = f.previous.pieces.remove(key) {
                f.current.pieces.insert(*key, piece);
            }
        }
        true
    }

    fn put_list(&self, list: u64, keys: &[u64]) {
        let f = &mut *self.0.lock();
        f.used = true;
        if f.current.lists.get(&list).is_some_and(|k| **k == *keys) {
            return;
        }
        if let Some(old) = f.previous.lists.remove(&list)
            && *old == *keys
        {
            f.current.lists.insert(list, old);
            return;
        }
        f.current.lists.insert(list, keys.into());
    }

    /// End of a window frame: pieces unused for a whole active frame go.
    pub(crate) fn finish_frame(&self) {
        let f = &mut *self.0.lock();
        if std::mem::take(&mut f.used) {
            std::mem::swap(&mut f.previous, &mut f.current);
            f.current.pieces.clear();
            f.current.lists.clear();
        }
    }

    /// Fonts changed: no piece is valid any more.
    pub(crate) fn clear(&self) {
        *self.0.lock() = Frames::default();
    }
}

impl WindowTextSystem {
    /// `layout_line_spaced` (no forced width) whose miss is laid out by
    /// `compose` instead of the platform shaper: the composed line is cached
    /// under the key of the whole text. `compose` must give the line the
    /// shaper would (glyph positions from the start of the line, tracking
    /// applied) and must not lay out lines through this text system.
    pub fn layout_line_composed(
        &self,
        text: &str,
        font_size: Pixels,
        runs: &[TextRun],
        letter_spacing: Pixels,
        compose: impl FnOnce() -> LineLayout,
    ) -> Arc<LineLayout> {
        let mut font_runs = self.font_runs_pool.lock().pop().unwrap_or_default();
        font_runs.clear();
        self.fill_font_runs(runs, font_size, &mut font_runs);
        let text = self.no_break_hyphens(text, &font_runs);
        let layout = self.line_layout_cache.layout_line_with(
            &SharedString::new(&*text),
            font_size,
            &font_runs,
            None,
            letter_spacing,
            |_, _| compose(),
        );
        self.font_runs_pool.lock().push(font_runs);
        layout
    }

    /// The piece stored under `key`, kept for this frame.
    pub fn line_piece(&self, key: u64) -> Option<LinePiece> {
        self.line_layout_cache.pieces.get(key)
    }

    /// Store a piece of a composed line under `key`.
    pub fn put_line_piece(&self, key: u64, piece: LinePiece) {
        self.line_layout_cache.pieces.put(key, piece);
    }

    /// Store the keys of pieces a caller keeps together under `list`.
    pub fn put_line_piece_list(&self, list: u64, keys: &[u64]) {
        self.line_layout_cache.pieces.put_list(list, keys);
    }

    /// Keep the pieces listed under `list` for this frame; `false` when the
    /// list is gone.
    pub fn keep_line_pieces(&self, list: u64) -> bool {
        self.line_layout_cache.pieces.keep(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px;

    fn piece(width: f32) -> LinePiece {
        LinePiece {
            layout: Arc::default(),
            runs: 0..0,
            base: 0,
            origin: px(0.),
            width: px(width),
            ascent: px(0.),
            descent: px(0.),
        }
    }

    #[test]
    fn pieces_age_only_by_frames_that_use_them() {
        let cache = PieceCache::default();
        cache.put(1, piece(1.));
        cache.put(2, piece(2.));
        cache.put_list(9, &[1]);
        cache.finish_frame();
        // Idle frames keep everything.
        cache.finish_frame();
        cache.finish_frame();
        assert!(cache.keep(9), "the list survives idle frames");
        cache.finish_frame();
        assert_eq!(cache.get(1).map(|p| p.width), Some(px(1.)), "kept by its list");
        assert!(cache.get(2).is_none(), "unused for a whole active frame");
        cache.clear();
        assert!(!cache.keep(9));
    }
}