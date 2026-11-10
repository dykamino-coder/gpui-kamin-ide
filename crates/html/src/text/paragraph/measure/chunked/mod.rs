//! Длинный кусок, набранный частями (`chunks`), — один `Seg` с одним набором:
//! прогоны глифов частей идут подряд, глифы каждой части стоят от её начала
//! (так часть переносится в другой кусок точно, без округления сдвига), а
//! положения знаков (`positions_with`) прибавляют к ним начало части. Набор
//! куска хранится в памяти строк текстовой системы под ключом всего его
//! текста, отдельно от наборов целиком (`layout_line_composed`): повторный
//! замер того же текста — в любом документе окна — берёт его без набора.
//! Части лежат там же (`LinePiece`: ссылка на набор куска с диапазоном
//! прогонов), поэтому правка куска копирует глифы неизменных частей и
//! перенабирает только задетые.

use super::chunks::{Cuts, tracking};
use crate::text::paragraph::*;
use gpui::{LineLayout, LinePiece, Pixels, ShapedGlyph, ShapedRun, Window, WindowTextSystem, px};
use std::sync::Arc;

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
        let mut layout = text.layout_line_composed(
            &self.text[start..end],
            self.font_size,
            &runs,
            tracking(self),
            || {
                composed = true;
                self.compose(start, end, ends, piece_keys, &mut pieces, &text)
            },
        );
        // Набор из памяти: ширины частей (они зависят только от ключа) дают
        // их начала; без частей в памяти кусок собирается заново.
        if !composed && !widths(piece_keys, &mut pieces, &text) {
            layout = Arc::new(self.compose(start, end, ends, piece_keys, &mut pieces, &text));
            composed = true;
        }
        if composed {
            for (&key, p) in piece_keys.iter().zip(&pieces) {
                let piece = LinePiece {
                    layout: layout.clone(),
                    runs: p.runs.0..p.runs.1,
                    base: p.base,
                    width: p.width,
                    ascent: p.ascent,
                    descent: p.descent,
                };
                text.put_line_piece(key, piece);
            }
        }
        keys.len += count;
        let seg = Seg {
            positions: Default::default(),
            start,
            end,
            layout,
            offset,
        };
        // Положения считаются сразу: начала частей известны только здесь.
        let mut origins = [px(0.); PIECES];
        for i in 1..count {
            origins[i] = origins[i - 1] + pieces[i - 1].width;
        }
        let (mut at, mut piece) = (0, 0);
        let positions = self.positions_with(&seg, |run| {
            if let Some(g) = seg.layout.runs[run].glyphs.first() {
                at = g.index;
            }
            while piece + 1 < count && at >= ends[piece] - start {
                piece += 1;
            }
            Some(origins[piece])
        });
        _ = seg.positions.set(positions);
        Some(seg)
    }
}

/// Ширины частей `keys` из памяти (части продлеваются на кадр); `false`,
/// если какой-то нет.
fn widths(keys: &[u64], pieces: &mut [Piece; PIECES], text: &WindowTextSystem) -> bool {
    for (p, &key) in pieces.iter_mut().zip(keys) {
        match text.line_piece(key) {
            Some(c) => {
                *p = Piece {
                    width: c.width,
                    ..*p
                }
            }
            None => return false,
        }
    }
    true
}
