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
    key: u64,
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
        let mut next = Some(cuts.next(self, window)?);
        let text = window.text_system().with_ligature_breaking(false);
        let mut runs = Vec::with_capacity(self.run_indices(&(start..end)).len());
        let mut pieces = [Piece::default(); PIECES];
        let (mut count, mut from, mut origin) = (0, start, px(0.));
        loop {
            let to = next.unwrap_or(end);
            let key = self.chunk_key(&(from..to));
            let base = from - start;
            let (width, ascent, descent) = match self.measures.chunk(key) {
                Some(super::Shaped::Chunk(c)) => c.copy_into(base, &mut runs),
                _ => {
                    let at = self.run_indices(&(from..to));
                    let skip = at.clone().next().map_or(0, |i| {
                        from - (self.prepared().run_ends[i] - self.runs[i].len)
                    });
                    text.shape_into(
                        &self.text[from..to],
                        self.font_size,
                        &self.runs[at],
                        skip,
                        tracking(self),
                        base,
                        &mut runs,
                    )
                }
            };
            pieces[count] = Piece {
                key,
                base: base as u32,
                runs_end: runs.len() as u32,
                width,
                ascent,
                descent,
                origin,
            };
            count += 1;
            origin += width;
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
        let pieces = &pieces[..count];
        let layout = Arc::new(LineLayout {
            font_size: self.font_size,
            width: origin,
            ascent: pieces.iter().map(|p| p.ascent).fold(px(0.), Pixels::max),
            descent: pieces.iter().map(|p| p.descent).fold(px(0.), Pixels::max),
            runs,
            len: end - start,
        });
        let mut runs_start = 0;
        for piece in pieces {
            let chunk = Chunk {
                layout: layout.clone(),
                runs: runs_start..piece.runs_end,
                base: piece.base,
                width: piece.width,
                ascent: piece.ascent,
                descent: piece.descent,
            };
            runs_start = piece.runs_end;
            self.measures
                .put_chunk(piece.key, super::Shaped::Chunk(chunk));
            keys.keys[keys.len] = piece.key;
            keys.len += 1;
        }
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

impl Chunk {
    /// Глифы части — в набор куска, где она начинается со смещения `base`.
    fn copy_into(&self, base: usize, out: &mut Vec<ShapedRun>) -> (Pixels, Pixels, Pixels) {
        let from = self.runs.start as usize..self.runs.end as usize;
        let shift = |index: usize| index - self.base as usize + base;
        out.extend(self.layout.runs[from].iter().map(|run| {
            ShapedRun {
                font_id: run.font_id,
                font_size: run.font_size,
                glyphs: run
                    .glyphs
                    .iter()
                    .map(|g| ShapedGlyph {
                        id: g.id,
                        position: g.position,
                        index: shift(g.index),
                        is_emoji: g.is_emoji,
                    })
                    .collect(),
            }
        }));
        (self.width, self.ascent, self.descent)
    }
}
