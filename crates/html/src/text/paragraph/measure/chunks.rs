//! Точки разреза длинного куска для набора частями (модель словного кэша
//! Gecko и `ReusingTextShaper` Blink: правка внутри абзаца перенабирает одну
//! часть, остальные берутся из памяти документа по ключу своего текста и
//! шрифтов, `chunked`).
//!
//! Режется только там, где набор целого куска и так рвётся: на границе
//! прогонов, которые текстовая система набирает порознь (другая грань, кегль
//! или возможности шрифта — `shapes_apart`), поэтому ни кернинг, ни лигатура,
//! ни контекстная форма через точку не проходят, а глифы части те же, что в
//! наборе целиком. По UAX #9 точка нейтральна: она вне всех явных вложений
//! абзаца ltr, и с обеих сторон латинская буква (или пробел при ней), так что
//! разрешение уровней от разреза не зависит. Из таких точек берутся те, где
//! хэш слова перед точкой кратен 128 (или часть уже длиннее `MAX`): точки
//! заданы текстом рядом, и правка сдвигает лишь границы своей части.

use super::bidi_levels::Levels;
use crate::text::paragraph::*;
use gpui::{Window, px};
use std::ops::Range;

/// Наименьшая часть: кусок короче двух таких набирается целиком.
const MIN: usize = 4096;
/// Часть длиннее режется в первой же годной точке.
const MAX: usize = 16384;

/// Поиск точек разреза куска `from..end` слева направо.
pub(super) struct Cuts {
    from: usize,
    end: usize,
    /// Следующий байт для разбора явных уровней.
    at: usize,
    /// Следующий прогон, чей конец — кандидат.
    run: usize,
    levels: Levels,
}

impl Paragraph {
    /// Поиск точек для куска `start..end` или `None`, если кусок набирается целиком.
    pub(super) fn cuts(&self, start: usize, end: usize) -> Option<Cuts> {
        (end - start >= MIN * 2 && !self.wrap.rtl && !self.vertical).then(|| Cuts {
            from: start,
            end,
            at: start,
            run: self.run_indices(&(start..end)).start,
            levels: Levels::default(),
        })
    }

    /// Ключ набора части: её текст, кегль, трекинг и шрифты прогонов.
    pub(super) fn chunk_key(&self, r: &Range<usize>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text[r.clone()].hash(&mut h);
        f32::from(self.font_size).to_bits().hash(&mut h);
        f32::from(self.letter_spacing).to_bits().hash(&mut h);
        self.joined_spacing().hash(&mut h);
        for i in self.run_indices(r) {
            let (run, end) = (&self.runs[i], self.prepared().run_ends[i]);
            let len = end.min(r.end).saturating_sub((end - run.len).max(r.start));
            if len == 0 {
                continue;
            }
            len.hash(&mut h);
            run.font_size.map(|s| f32::from(s).to_bits()).hash(&mut h);
            run.font.family.hash(&mut h);
            run.font.weight.0.to_bits().hash(&mut h);
            (run.font.style as u8).hash(&mut h);
            for (tag, value) in run.font.features.tag_value_list() {
                tag.hash(&mut h);
                value.hash(&mut h);
            }
        }
        h.finish()
    }
}

impl Cuts {
    /// Следующая точка разреза или `None`: дальше кусок идёт одной частью.
    pub(super) fn next(&mut self, p: &Paragraph, window: &mut Window) -> Option<usize> {
        let bytes = p.text.as_bytes();
        let ends = &p.prepared().run_ends;
        while self.run < ends.len() {
            let (i, c) = (self.run, ends[self.run]);
            self.run += 1;
            if c + MIN > self.end {
                self.run = ends.len();
                break;
            }
            while self.at < c {
                if !self.levels.step(bytes, self.at) {
                    self.run = ends.len();
                    return None;
                }
                self.at += 1;
            }
            if c < self.from + MIN || p.runs[i].len == 0 || !self.levels.zero() || !quiet(bytes, c)
            {
                continue;
            }
            if !word_hash(bytes, c).is_multiple_of(128) && c - self.from < MAX {
                continue;
            }
            let Some(next) = (i + 1..ends.len()).find(|&j| p.runs[j].len > 0) else {
                continue;
            };
            let size = |k: usize| p.runs[k].font_size.unwrap_or(p.font_size);
            let (a, b) = (&p.runs[i], &p.runs[next]);
            if window
                .text_system()
                .shapes_apart(&a.font, size(i), &b.font, size(next))
            {
                self.from = c;
                return Some(c);
            }
        }
        None
    }
}

/// Точка `c` нейтральна для разрешения уровней: по обе стороны латинская
/// буква (сильная L по UAX #9) либо пробел при ней (слева — и после знака препинания).
fn quiet(bytes: &[u8], c: usize) -> bool {
    // Знак препинания между буквой и пробелом (CS/ON) чисел рядом не имеет
    // (W4 их не касается) и разрешается по буквам вокруг (N1), как и пробел.
    let punct = |b: u8| matches!(b, b'.' | b',' | b';' | b':' | b'!' | b'?');
    let left = bytes[c - 1].is_ascii_alphabetic()
        || (bytes[c - 1] == b' '
            && (bytes[c - 2].is_ascii_alphabetic()
                || (punct(bytes[c - 2]) && bytes[c - 3].is_ascii_alphabetic())));
    let right = bytes[c].is_ascii_alphabetic()
        || (bytes[c] == b' ' && bytes.get(c + 1).is_some_and(u8::is_ascii_alphabetic));
    left && right
}

/// Хэш восьми байт перед точкой `c`: точки зависят только от текста рядом.
fn word_hash(bytes: &[u8], c: usize) -> u32 {
    bytes[c - 8..c].iter().fold(2_166_136_261u32, |h, &b| {
        (h ^ u32::from(b)).wrapping_mul(16_777_619)
    })
}

/// Трекинг, который набор применяет сам (при совместном пути абзац добавляет его позже).
pub(super) fn tracking(p: &Paragraph) -> gpui::Pixels {
    if p.joined_spacing() {
        px(0.)
    } else {
        p.letter_spacing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_are_neutral_and_follow_the_text_not_the_offset() {
        let text = b"alpha beta gamma delta";
        assert!(quiet(text, 2), "inside a word");
        assert!(quiet(text, 5), "before the space");
        assert!(quiet(text, 6), "after the space");
        assert!(!quiet(b"a1 b2 3c", 3), "a digit is not a strong letter");
        assert!(!quiet(b"ab, cd", 3), "punctuation is not a strong letter");
        assert!(
            quiet(b"ab, cd", 4),
            "after a letter, punctuation and a space"
        );
        assert!(!quiet(b"a1, cd", 4), "after a digit");
        let shifted = b"xxalpha beta gamma delta";
        assert_eq!(word_hash(text, 11), word_hash(shifted, 13));
    }
}
