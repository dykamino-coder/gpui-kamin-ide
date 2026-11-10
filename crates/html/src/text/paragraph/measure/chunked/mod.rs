//! Длинный кусок, набранный частями (`chunks`), — один `Seg` с одним набором,
//! неотличимым от набора целиком: прогоны глифов частей идут подряд, глифы
//! стоят от начала куска. Набор куска хранится в памяти строк текстовой
//! системы под ключом всего его текста (`layout_line_composed`), как набор
//! целиком: повторный замер того же текста — в любом документе окна — берёт
//! его без набора. Части лежат там же (`LinePiece`: ссылка на набор куска с
//! диапазоном прогонов), поэтому правка куска копирует глифы неизменных
//! частей со сдвигом и перенабирает только задетые.

use super::chunks::{Cuts, tracking};
use crate::text::paragraph::*;
use gpui::{LineLayout, LinePiece, Pixels, ShapedGlyph, ShapedRun, Window, WindowTextSystem, px};

mod compose;

/// Наибольшее число частей одного абзаца; дальше кусок не режется.
pub(super) const PIECES: usize = 128;

/// Ключи частей одного замера (без памяти в куче).
pub(super) struct Keys {
    pub(super) keys: [u64; PIECES],
    pub(super) len: usize,
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            keys: [0; PIECES],
            len: 0,
        }
    }
}

/// Часть в собранном наборе куска.
#[derive(Clone, Copy, Default)]
struct Piece {
    runs: (u32, u32),
    base: u32,
    origin: Pixels,
    width: Pixels,
    ascent: Pixels,
    descent: Pixels,
}

impl Paragraph {
    /// Кусок `start..end` со сдвигом `offset`, набранный частями, или `None`,
    /// если точек разреза нет. Ключи его частей добавляются в `keys`.
    pub(super) fn chunked_seg(
        &self,
        start: usize,
        end: usize,
        offset: Pixels,
        keys: &mut Keys,
        window: &mut Window,
    ) -> Option<Seg> {
        let mut cuts: Cuts = self.cuts(start, end)?;
        if keys.len + 2 > PIECES {
            return None;
        }
        let mut ends = [0usize; PIECES];
        let mut next = Some(cuts.next(self, window)?);
        let (mut count, mut from) = (0, start);
        loop {
            let to = next.unwrap_or(end);
            (ends[count], keys.keys[keys.len + count]) = (to, self.chunk_key(&(from..to)));
            count += 1;
            if to == end {
                break;
            }
            from = to;
            next = if keys.len + count + 1 < PIECES {
                cuts.next(self, window)
            } else {
                None
            };
        }
        let (ends, piece_keys) = (&ends[..count], &keys.keys[keys.len..keys.len + count]);
        let text = window.text_system().with_ligature_breaking(false);
        // Ключ набора — как у набора куска целиком: те же текст и прогоны.
        let runs = self.own_runs(&(start..end));
        let mut pieces = [Piece::default(); PIECES];
        let mut composed = false;
        let layout = text.layout_line_composed(
            &self.text[start..end],
            self.font_size,
            &runs,
            tracking(self),
            || {
                composed = true;
                self.compose(start, end, ends, piece_keys, &mut pieces, &text)
            },
        );
        for (&key, p) in piece_keys.iter().zip(&pieces) {
            if composed {
                let piece = LinePiece {
                    layout: layout.clone(),
                    runs: p.runs.0..p.runs.1,
                    base: p.base,
                    origin: p.origin,
                    width: p.width,
                    ascent: p.ascent,
                    descent: p.descent,
                };
                text.put_line_piece(key, piece);
            } else {
                // Набор из памяти: его части живут столько же, сколько он.
                _ = text.line_piece(key);
            }
        }
        keys.len += count;
        Some(Seg {
            positions: Default::default(),
            start,
            end,
            layout,
            offset,
        })
    }
}
