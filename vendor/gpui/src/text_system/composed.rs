//! KaminIDE patch: a line laid out from pieces shaped apart. A caller that
//! cuts a long line where the shaper breaks it anyway (no kerning, ligature or
//! contextual form across the cut) composes the line from its pieces. The
//! glyphs of each piece keep their positions from the start of the piece, so
//! a piece moves into another line exactly, without rounding: such a line is
//! not a shaped line and is cached apart from shaped lines, under the key of
//! its whole text. Its pieces are kept by the caller's keys, so an edit of the
//! text reshapes only the pieces it touches and copies the glyphs of the others.
//!
//! Pieces live in one store per process (font ids belong to the process text
//! system) and age with the window frames like cached lines: a piece no frame
//! used for two frames is dropped, and the store frees its tables once empty.

use super::{LineLayout, TextRun, WindowTextSystem};
use crate::{Pixels, SharedString, px};
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
    /// Advance of the piece (letter-spacing included).
    pub width: Pixels,
    /// Ascent of the piece's runs.
    pub ascent: Pixels,
    /// Descent of the piece's runs.
    pub descent: Pixels,
}

/// Wrap width under which composed lines are cached: no shaped line uses it.
pub(crate) const COMPOSED: Option<Pixels> = Some(px(-1.));

#[derive(Default)]
struct Frame {
    pieces: FxHashMap<u64, LinePiece>,
    lists: FxHashMap<u64, Arc<[u64]>>,
}

impl Frame {
    fn is_empty(&self) -> bool {
        self.pieces.is_empty() && self.lists.is_empty()
    }
}

#[derive(Default)]
struct Frames {
    current: Frame,
    previous: Frame,
}

/// Pieces of composed lines used in the current and the previous frame.
pub(crate) struct PieceCache(Mutex<Option<Frames>>);

/// The pieces of every window of the process.
pub(crate) static PIECES: PieceCache = PieceCache(parking_lot::const_mutex(None));

impl PieceCache {
    fn get(&self, key: u64) -> Option<LinePiece> {
        let mut f = self.0.lock();
        let f = f.as_mut()?;
        if let Some(piece) = f.current.pieces.get(&key) {
            return Some(piece.clone());
        }
        let piece = f.previous.pieces.remove(&key)?;
        f.current.pieces.insert(key, piece.clone());
        Some(piece)
    }

    fn put(&self, key: u64, piece: LinePiece) {
        let mut f = self.0.lock();
        f.get_or_insert_default().current.pieces.insert(key, piece);
    }

    /// Keep the list `list` and its pieces for this frame; `false` when it is gone.
    fn keep(&self, list: u64) -> bool {
        let mut f = self.0.lock();
        let Some(f) = f.as_mut() else {
            return false;
        };
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
        let mut f = self.0.lock();
        let f = f.get_or_insert_default();
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

    /// End of a window frame: pieces unused for two frames go.
    pub(crate) fn finish_frame(&self) {
        let mut guard = self.0.lock();
        let Some(f) = guard.as_mut() else {
            return;
        };
        std::mem::swap(&mut f.previous, &mut f.current);
        f.current.pieces.clear();
        f.current.lists.clear();
        if f.previous.is_empty() {
            *guard = None;
        }
    }

    /// Fonts changed: no piece is valid any more.
    pub(crate) fn clear(&self) {
        *self.0.lock() = None;
    }
}

impl WindowTextSystem {
    /// A line composed by `compose` from pieces shaped apart, cached under the
    /// key of the whole text apart from shaped lines. `compose` gives the
    /// glyphs of each piece positions from the start of the piece (tracking
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
            COMPOSED,
            None,
            letter_spacing,
            |_, _| compose(),
        );
        self.font_runs_pool.lock().push(font_runs);
        layout
    }

    /// The piece stored under `key`, kept for this frame.
    pub fn line_piece(&self, key: u64) -> Option<LinePiece> {
        PIECES.get(key)
    }

    /// Store a piece of a composed line under `key`.
    pub fn put_line_piece(&self, key: u64, piece: LinePiece) {
        PIECES.put(key, piece);
    }

    /// Store the keys of pieces a caller keeps together under `list`.
    pub fn put_line_piece_list(&self, list: u64, keys: &[u64]) {
        PIECES.put_list(list, keys);
    }

    /// Keep the pieces listed under `list` for this frame; `false` when the
    /// list is gone.
    pub fn keep_line_pieces(&self, list: u64) -> bool {
        PIECES.keep(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(width: f32) -> LinePiece {
        LinePiece {
            layout: Arc::default(),
            runs: 0..0,
            base: 0,
            width: px(width),
            ascent: px(0.),
            descent: px(0.),
        }
    }

    #[test]
    fn pieces_age_with_frames_and_lists_keep_them() {
        let cache = PieceCache(parking_lot::const_mutex(None));
        cache.put(1, piece(1.));
        cache.put(2, piece(2.));
        cache.put_list(9, &[1]);
        cache.finish_frame();
        assert!(cache.keep(9), "the list survives one frame");
        cache.finish_frame();
        assert_eq!(cache.get(1).map(|p| p.width), Some(px(1.)), "kept by its list");
        assert!(cache.get(2).is_none(), "unused for two frames");
        cache.finish_frame();
        cache.finish_frame();
        assert!(cache.0.lock().is_none(), "an empty store frees its tables");
        assert!(!cache.keep(9));
    }
}
