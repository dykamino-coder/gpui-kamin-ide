//! Длинный кусок, набранный частями (`chunks`), — один `Seg` с одним набором,
//! как у куска, набранного целиком: прогоны глифов частей идут подряд, глифы
//! каждой части стоят от её начала (так часть переносится в другой кусок без
//! пересчёта координат — сдвиг на целое смещение байтов точен), а положения
//! знаков (`positions_with`) прибавляют к ним начало части. Набор части
//! хранится в памяти документа как ссылка на набор её куска с диапазоном
//! прогонов: память глифов не удваивается, а правка куска копирует глифы
//! неизменных частей вместо их перенабора. Набор частей идёт мимо памяти
//! строк текстовой системы: свой текст и свои прогоны она не копирует.

use super::chunks::{Cuts, tracking};
use crate::text::paragraph::*;
use gpui::{LineLayout, Pixels, ShapedGlyph, ShapedRun, Window, px};
use std::{ops::Range, rc::Rc, sync::Arc};

mod compose;

/// Наибольшее число частей одного абзаца; дальше кусок не режется.
pub(super) const PIECES: usize = 128;

/// Набор одной части: её прогоны в наборе куска `layout`.
#[derive(Clone)]
pub(in crate::text::paragraph) struct Chunk {
    layout: Arc<LineLayout>,
    runs: Range<u32>,
    /// Смещение части от начала её куска (индексы её глифов считаются от него).
    base: u32,
    width: Pixels,
    ascent: Pixels,
    descent: Pixels,
}

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

#[derive(Clone, Copy, Default)]
struct Piece {
    base: u32,
    runs_end: u32,
    width: Pixels,
    ascent: Pixels,
    descent: Pixels,
    origin: Pixels,
}

impl Paragraph {
    /// Кусок `start..end` со сдвигом `offset`, набранный частями, или `None`,
    /// если точек разреза нет.
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
        // Первый проход: точки, ключи частей и их наборы в памяти.
        let mut ends = [0usize; PIECES];
        let mut found: [Option<Chunk>; PIECES] = std::array::from_fn(|_| None);
        let mut next = Some(cuts.next(self, window)?);
        let (mut count, mut from) = (0, start);
        loop {
            let to = next.unwrap_or(end);
            let key = self.chunk_key(&(from..to));
            (ends[count], keys.keys[keys.len + count]) = (to, key);
            if let Some(super::Shaped::Chunk(c)) = self.measures.chunk(key) {
                found[count] = Some(c);
            }
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
        let found = &found[..count];
        let mut pieces = [Piece::default(); PIECES];
        let layout = match self.same_layout(start, end, &ends[..count], found) {
            // Тот же кусок (другой замер того же текста): набор общий.
            Some(layout) => {
                let mut origin = px(0.);
                for (piece, c) in pieces.iter_mut().zip(found.iter().flatten()) {
                    *piece = c.piece(origin);
                    origin += c.width;
                }
                layout
            }
            None => self.compose(start, end, &ends[..count], found, &mut pieces, window),
        };
        let pieces = &pieces[..count];
        let mut runs_start = 0;
        for (i, piece) in pieces.iter().enumerate() {
            let key = keys.keys[keys.len + i];
            let chunk = Chunk {
                layout: layout.clone(),
                runs: runs_start..piece.runs_end,
                base: piece.base,
                width: piece.width,
                ascent: piece.ascent,
                descent: piece.descent,
            };
            runs_start = piece.runs_end;
            self.measures.put_chunk(key, super::Shaped::Chunk(chunk));
        }
        keys.len += count;
        let seg = Seg {
            positions: Rc::default(),
            start,
            end,
            layout,
            offset,
        };
        // Положения считаются сразу: начала частей известны только здесь.
        let mut at = 0;
        let positions = self.positions_with(&seg, |run| {
            while at + 1 < pieces.len() && run >= pieces[at].runs_end as usize {
                at += 1;
            }
            Some(pieces[at].origin)
        });
        _ = seg.positions.set(positions);
        Some(seg)
    }
}
